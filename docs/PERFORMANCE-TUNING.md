# ChronoSeal Performance Tuning Guide

ChronoSeal uses a deterministic Synthetic Gene Mutation Engine to strengthen browser session continuity validation. This guide explains how to tune the mutation engine for an appropriate balance between security strength, resource consumption, and user experience.

The primary tuning parameters are:

* `gene_size` — size of the synthetic gene buffer
* `mutation_rounds` — number of mutation iterations executed per heartbeat

---

## Understanding the Mutation Engine

For every accepted heartbeat, ChronoSeal executes a server-generated mutation program against a synthetic gene buffer.

Increasing mutation complexity raises the computational cost of reproducing valid session state while also increasing CPU utilization on both the server and browser runtime.

General effects:

* Larger `gene_size` increases mutation state complexity.
* Higher `mutation_rounds` increase computational work per heartbeat.
* Both increase memory access and CPU consumption.
* Excessive values may negatively impact lower-end mobile devices.

The optimal values depend on your threat model and expected client hardware.

---

## Recommended Configurations

| Profile           | `gene_size` | `mutation_rounds` | Security Level   | Recommended Usage                |
| ----------------- | ----------- | ----------------- | ---------------- | -------------------------------- |
| Default           | 512         | 4                 | Moderate         | Development and testing          |
| Recommended       | 2048        | 4                 | Strong           | Most production deployments      |
| High Security     | 4096        | 8                 | Very Strong      | Sensitive applications           |
| Maximum Practical | 4096        | 10                | Extremely Strong | High-value targets               |
| Experimental      | 4096        | 10                | Research Only    | Benchmarking and experimentation |

### Recommended Production Configuration

```toml
gene_size = 2048
mutation_rounds = 4
```

This configuration provides a strong balance between security and runtime overhead for most deployments.

---

## Configuration

Edit your configuration file:

```toml
# Mutation Engine Settings

gene_size = 2048
mutation_rounds = 4
```

Common configuration locations:

```text
/etc/chronoseal/config.toml
~/.config/chronoseal/config.toml
```

Restart ChronoSeal:

```bash
sudo systemctl restart chronoseal
```

Validate the effective configuration:

```bash
chronoseal config check --format yaml
```

---

## Performance Monitoring

### Server-Side Monitoring

View service logs:

```bash
sudo journalctl -u chronoseal -f
```

Inspect runtime statistics:

```bash
chronoseal stats --format json
```

Enable additional diagnostics when required:

```bash
CHRONOSEAL_LOG=debug chronoseal run
```

Avoid debug logging in production environments.

---

### Browser-Side Monitoring

Measure mutation execution time:

```javascript
console.time("gene-mutation");

const commitment = preview_gene_commitment(
    order_b64,
    session_id,
    mutation_step,
    mutation_rounds
);

console.timeEnd("gene-mutation");
```

Browser developer tools can also be used to monitor:

* JavaScript execution time
* WASM execution time
* CPU utilization
* Memory consumption

---

## Tuning Strategy

### Step 1: Start with Recommended Values

```toml
gene_size = 2048
mutation_rounds = 4
```

Deploy and observe normal usage patterns.

### Step 2: Monitor Heartbeat Success Rates

Watch for:

* heartbeat failures
* increased browser CPU usage
* elevated mobile device latency
* increased battery consumption

### Step 3: Increase Gradually

Increase one parameter at a time.

Recommended progression:

```text
2048 / 4
4096 / 4
4096 / 8
4096 / 10
```

This makes it easier to identify performance bottlenecks.

### Step 4: Test Mobile Devices

Always test on:

* Android devices
* iPhones
* older laptops
* low-power CPUs

Desktop-only validation can be misleading.

---

## Advanced Deployment Strategies

### Risk-Based Mutation Strength

Future deployments may choose to dynamically increase mutation strength based on:

* session age
* failed heartbeat history
* suspicious behavior signals
* protected resource sensitivity

Example policy:

```text
New session          → 2048 / 4
Suspicious session   → 4096 / 8
Elevated-risk action → 4096 / 10
```

---

## Performance Recommendations

### WASM Builds

Always use optimized builds:

```bash
wasm-pack build wasm --target web --release
```

### General Guidance

* Keep `mutation_rounds` at or below 10 for all deployments.
* Prefer increasing `gene_size` before dramatically increasing rounds.
* Benchmark on representative client hardware.
* Monitor browser CPU utilization during load testing.
* Re-evaluate settings after major algorithm changes.

### Storage Performance

For maximum throughput:

```toml
db_type = "sqlite-in-memory"
```

For persistence:

```toml
db_type = "sqlite-in-disk"
```

Choose based on operational requirements rather than mutation settings.

---

## Security Considerations

Higher values increase the cost of reproducing valid session state but do not provide absolute protection against determined attackers.

ChronoSeal remains a cost-raising attestation layer rather than a complete anti-abuse solution.

Mutation tuning should be considered alongside:

* heartbeat timing controls
* signature validation
* hash-chain continuity
* behavioral trust checks
* rate limiting
* session expiration

---

## Recommended Starting Point

For most production deployments:

```toml
gene_size = 2048
mutation_rounds = 4
```

This configuration provides a strong balance between security, performance, and compatibility across desktop and mobile devices.

---

## Related Documentation

* `docs/ARCHITECTURE.md`
* `docs/API.md`
* `docs/THREAT_MODEL.md`
* `docs/REFRACTORING-v0.6.0.md`

For diagnostics:

```bash
chronoseal config check
chronoseal stats
chronoseal health
```


## v1.0.2 Limits

Current supported range:

```toml
gene_size = 1..=4096
mutation_rounds = 1..=10
```
