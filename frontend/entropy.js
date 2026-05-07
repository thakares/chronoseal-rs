const events = [];

document.addEventListener('mousemove', (e) => {
    events.push({ x: e.clientX, y: e.clientY, t: performance.now() });
    if (events.length > 300) events.shift();
});

export function collectEntropy(since) {
    return events.filter(e => e.t > since);
}