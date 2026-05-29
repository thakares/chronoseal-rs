import init, {
    generate_keypair,
    sign_message,
    compute_next_hash,
    run_program,
    init_gene_state,
    preview_gene_commitment,
    commit_gene_preview,
    discard_gene_preview
} from './pkg/chronoseal_wasm.js';
import { collectEntropy } from './entropy.js';
import { sendRequest } from './transport.js';

let session, prevHash, currentSalt, opcodesB64, lastTime;
let minInterval = 12000;
let maxInterval = 25000;
let pendingMutationStep = 0;
let pendingMutationOrderB64 = '';

export async function initHeartbeat() {
    await init();
    const pubHex = generate_keypair();
    const initResp = await sendRequest('/init', 'POST', { public_key: pubHex });
    session = initResp.session_id;
    prevHash = initResp.initial_hash;
    currentSalt = initResp.salt;
    opcodesB64 = initResp.opcodes_b64;
    minInterval = initResp.heartbeat_min_interval_ms || 12000;
    maxInterval = initResp.heartbeat_max_interval_ms || 25000;
    if (!init_gene_state(initResp.gene_size || 512)) {
        throw new Error('Failed to initialize gene state');
    }
    pendingMutationStep = initResp.mutation_step;
    pendingMutationOrderB64 = initResp.mutation_order_b64;
    lastTime = performance.now();
    scheduleNext();
}

function scheduleNext() {
    const delay = minInterval + Math.random() * (maxInterval - minInterval);
    setTimeout(sendHeartbeat, delay);
}

async function sendHeartbeat() {
    try {
        const now = performance.now();
        const events = collectEntropy(lastTime);
        lastTime = now;

        const stackState = JSON.stringify(run_program(opcodesB64));
        const fingerprint = {
            aspectRatio: (screen.width / screen.height).toFixed(10),
            devicePixelRatio: String(window.devicePixelRatio),
            hardwareConcurrency: navigator.hardwareConcurrency || 1
        };
        const timestamp = Date.now();
        const entropyData = { events: events.map(e => ({ x: e.x, y: e.y, t: e.t })) };
        const entropyJson = JSON.stringify(entropyData);
        const geneCommitment = preview_gene_commitment(pendingMutationOrderB64);
        if (!geneCommitment) {
            throw new Error('Unable to compute mutation commitment');
        }

        const signable = {
            sessionId: session,
            prevHash: prevHash,
            timestamp: timestamp,
            entropyData: entropyData,
            stackState: JSON.parse(stackState),
            fingerprint: fingerprint,
            mutationStep: pendingMutationStep,
            geneCommitment: geneCommitment
        };
        const msg = JSON.stringify(signable, Object.keys(signable).sort());
        const sig = sign_message(msg);
        if (!sig) {
            discard_gene_preview();
            console.error('Keypair not initialised — skipping heartbeat');
            return;
        }
        const resp = await sendRequest('/hb', 'POST', {
            session_id: session,
            prev_hash: prevHash,
            timestamp,
            entropy_data: entropyData,
            stack_state: JSON.parse(stackState),
            fingerprint,
            mutation_step: pendingMutationStep,
            gene_commitment: geneCommitment,
            signature: sig
        });

        if (resp.next_salt && resp.next_mutation_step && resp.next_mutation_order_b64) {
            if (!commit_gene_preview()) {
                discard_gene_preview();
                throw new Error('Failed to commit local mutation preview');
            }
            // IMPORTANT: capture the salt that was active when this heartbeat was sent.
            // The server computes new_hash = H(prev, ts, entropy, stack, OLD_salt) and stores it,
            // then rotates to next_salt.  We must mirror that using the same old salt, then rotate.
            const sentSalt = currentSalt;
            currentSalt = resp.next_salt;
            prevHash = compute_next_hash(prevHash, timestamp, entropyJson, stackState, sentSalt);
            pendingMutationStep = resp.next_mutation_step;
            pendingMutationOrderB64 = resp.next_mutation_order_b64;
        } else {
            discard_gene_preview();
            console.warn('Heartbeat rejected');
        }
    } catch (e) {
        discard_gene_preview();
        console.error(e);
    } finally {
        scheduleNext();
    }
}
