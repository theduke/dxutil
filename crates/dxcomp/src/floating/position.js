const [id, triggerSelector, contentSelector] = await dioxus.recv();
const root = document.querySelector(`[data-dx-floating="${id}"]`);
if (!root) return;
window.dxFloating ??= new Map();
window.dxFloating.get(id)?.();
// A nested control owns its own popup, even while an ancestor is open.
const owned = selector => [...root.querySelectorAll(selector)].find(
    element => element.closest('[data-dx-floating]') === root
);
const trigger = triggerSelector ? owned(triggerSelector) : null;
const contextMenu = !triggerSelector;
if (!contextMenu && !trigger) return;
let popup;
let preferredSide;
let preferredMinWidth;
let point;
let appliedLeft, appliedTop;
let frame;
const abort = new AbortController();
const schedule = () => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(position);
};
const resize = new ResizeObserver(schedule);
if (trigger) resize.observe(trigger);
// Avoid observing our own style writes as fresh pointer coordinates.
const style = (name, value) => {
    if (popup.style[name] !== value) popup.style[name] = value;
};
function position() {
    const next = owned(contentSelector);
    if (next !== popup) {
        if (popup) {
            resize.unobserve(popup);
            if (!next && trigger && document.activeElement === document.body) trigger.focus();
        }
        popup = next;
        preferredSide = popup?.dataset.side ?? 'bottom';
        preferredMinWidth = popup ? parseFloat(getComputedStyle(popup).minWidth) || 0 : 0;
        point = undefined;
        appliedLeft = appliedTop = undefined;
        if (popup) resize.observe(popup);
    }
    if (!popup) return;
    if (contextMenu) {
        // These are already layout-viewport coordinates, including the
        // primitive's Safari visualViewport correction for a long press.
        const x = parseFloat(popup.style.left), y = parseFloat(popup.style.top);
        point = {
            x: popup.style.left !== appliedLeft && Number.isFinite(x) ? x : (point?.x ?? 0),
            y: popup.style.top !== appliedTop && Number.isFinite(y) ? y : (point?.y ?? 0),
        };
    }
    if (popup.dataset.state === 'closed') {
        if (popup.matches(':popover-open')) popup.hidePopover();
        return;
    }
    popup.setAttribute('popover', 'manual');
    popup.classList.add('dx-floating-content');
    if (!popup.matches(':popover-open')) {
        popup.showPopover();
        popup.querySelector('[data-autofocus]')?.focus();
    }
    const viewport = window.visualViewport;
    const left = viewport?.offsetLeft ?? 0;
    const top = viewport?.offsetTop ?? 0;
    const width = viewport?.width ?? innerWidth;
    const height = viewport?.height ?? innerHeight;
    const margin = 8, gap = contextMenu ? 0 : 6;
    const anchor = contextMenu
        ? {left: point.x, right: point.x, top: point.y, bottom: point.y}
        : trigger.getBoundingClientRect();
    const clamp = (value, min, max) => Math.max(min, Math.min(value, max));
    const space = {
        top: clamp(anchor.top - top - margin - gap, 0, height - margin * 2),
        bottom: clamp(top + height - anchor.bottom - margin - gap, 0, height - margin * 2),
        left: clamp(anchor.left - left - margin - gap, 0, width - margin * 2),
        right: clamp(left + width - anchor.right - margin - gap, 0, width - margin * 2),
    };
    style('maxWidth', `${Math.max(0, width - margin * 2)}px`);
    if (popup.getAttribute('role') === 'listbox') {
        style('minWidth', `${Math.max(0, Math.min(anchor.width, width - margin * 2))}px`);
    } else {
        style('minWidth', `${Math.max(0, Math.min(preferredMinWidth, width - margin * 2))}px`);
    }
    const vertical = preferredSide === 'top' || preferredSide === 'bottom';
    const opposite = {top: 'bottom', bottom: 'top', left: 'right', right: 'left'}[preferredSide];
    const heightLimit = contextMenu ? Math.max(0, height - margin * 2) : 360;
    const desired = vertical ? Math.min(popup.scrollHeight, heightLimit) : popup.getBoundingClientRect().width;
    const side = space[preferredSide] >= desired || space[preferredSide] >= space[opposite] ? preferredSide : opposite;
    style('maxHeight', `${Math.min(heightLimit, vertical ? space[side] : height - margin * 2)}px`);
    if (!vertical) style('maxWidth', `${space[side]}px`);
    const box = popup.getBoundingClientRect();
    const align = popup.dataset.align ?? 'start';
    const aligned = (start, end, size) => align === 'end' ? end - size : align === 'center' ? (start + end - size) / 2 : start;
    const x = vertical ? aligned(anchor.left, anchor.right, box.width) : side === 'right' ? anchor.right + gap : anchor.left - gap - box.width;
    const y = vertical ? (side === 'bottom' ? anchor.bottom + gap : anchor.top - gap - box.height) : aligned(anchor.top, anchor.bottom, box.height);
    appliedLeft = `${clamp(x, left + margin, left + width - box.width - margin)}px`;
    appliedTop = `${clamp(y, top + margin, top + height - box.height - margin)}px`;
    style('left', appliedLeft);
    style('top', appliedTop);
    popup.dataset.side = side;
}
const mutation = new MutationObserver(schedule);
mutation.observe(root, {
    childList: true, subtree: true, attributes: true,
    attributeFilter: contextMenu ? ['data-state', 'data-align', 'style'] : ['data-state', 'data-align'],
});
window.addEventListener('resize', schedule, { signal: abort.signal });
document.addEventListener('scroll', schedule, { capture: true, signal: abort.signal });
window.visualViewport?.addEventListener('resize', schedule, { signal: abort.signal });
window.visualViewport?.addEventListener('scroll', schedule, { signal: abort.signal });
window.dxFloating.set(id, () => {
    abort.abort();
    mutation.disconnect();
    resize.disconnect();
    cancelAnimationFrame(frame);
    if (popup?.matches(':popover-open')) popup.hidePopover();
    window.dxFloating.delete(id);
});
schedule();
