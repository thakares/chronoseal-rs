import init, { generate_keypair, sign_message, compute_next_hash, run_program } from './pkg/antibot_wasm.js';
import { collectEntropy } from './entropy.js';
import { sendRequest } from './transport.js';

let session, prevHash, currentSalt, opcodesB64, lastTime;

export async function initHeartbeat() {
    await init();
    const pubHex = generate_keypair();
    const initResp = await sendRequest('/init', 'POST', { public_key: pubHex });
    session = initResp.session_id;
    prevHash = initResp.initial_hash;
    currentSalt = initResp.salt;
    opcodesB64 = initResp.opcodes_b64;
    lastTime = performance.now();
    scheduleNext();
}

function scheduleNext() {
    const delay = 12000 + Math.random() * 13000;
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

        const signable = {
            sessionId: session,
            prevHash: prevHash,
            timestamp: timestamp,
            entropyData: entropyData,
            stackState: JSON.parse(stackState),
            fingerprint: fingerprint
        };
        const msg = JSON.stringify(signable, Object.keys(signable).sort());
        const sig = sign_message(msg);
        if (!sig) {
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
            signature: sig
        });

        if (resp.next_salt) {
            // IMPORTANT: capture the salt that was active when this heartbeat was sent.
            // The server computes new_hash = H(prev, ts, entropy, stack, OLD_salt) and stores it,
            // then rotates to next_salt.  We must mirror that using the same old salt, then rotate.
            const sentSalt = currentSalt;
            currentSalt = resp.next_salt;
            prevHash = compute_next_hash(prevHash, timestamp, entropyJson, stackState, sentSalt);
        } else {
            console.warn('Heartbeat rejected');
        }
    } catch (e) {
        console.error(e);
    } finally {
        scheduleNext();
    }
}