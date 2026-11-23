// Motion grammar for the parts view, built on anime.js.
// One tween for live numbers (linear, one progress tick long, so bars glide instead of jump),
// one entrance (exponential ease-out), one flash for a split. All off under reduced motion.
import { animate } from 'animejs';

const TICK = 260; // progress arrives every 250 ms

export const reducedMotion = () =>
  typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

type Box = { left: number; width: number };

/** Svelte action: keep an absolutely positioned element at `left`/`width` (percent), gliding between updates. */
export function box(node: HTMLElement, b: Box) {
  node.style.left = `${b.left}%`;
  node.style.width = `${b.width}%`;
  return {
    update(n: Box) {
      if (reducedMotion()) {
        node.style.left = `${n.left}%`;
        node.style.width = `${n.width}%`;
        return;
      }
      animate(node, { left: `${n.left}%`, width: `${n.width}%`, duration: TICK, ease: 'linear', composition: 'replace' });
    },
  };
}

/** Svelte action: glide one percentage property (`width` of a fill, `left` of a head). */
export function glide(node: HTMLElement, p: { prop: 'width' | 'left'; value: number }) {
  node.style[p.prop] = `${p.value}%`;
  return {
    update(n: { prop: 'width' | 'left'; value: number }) {
      if (reducedMotion()) {
        node.style[n.prop] = `${n.value}%`;
        return;
      }
      animate(node, { [n.prop]: `${n.value}%`, duration: TICK, ease: 'linear', composition: 'replace' });
    },
  };
}

/** Svelte action: a new row or panel arrives from just above, settling fast. */
export function arrive(node: HTMLElement, opts: { y?: number; delay?: number } = {}) {
  if (reducedMotion()) return;
  animate(node, { opacity: [0, 1], translateY: [opts.y ?? -10, 0], duration: 560, delay: opts.delay ?? 0, ease: 'outExpo' });
}

/**
 * Svelte action for list rows: a new row opens from zero height, so the rows below glide
 * down on their own (no competing transform animations).
 */
export function openRow(node: HTMLElement, height = 38) {
  if (reducedMotion()) return;
  node.style.overflow = 'hidden';
  animate(node, {
    height: [0, height],
    opacity: [0, 1],
    duration: 520,
    ease: 'outExpo',
    onComplete: () => {
      node.style.overflow = '';
      node.style.height = '';
    },
  });
}

/**
 * Svelte transition (outro): a finished row closes up. Driven per frame in JS (like the rest
 * of the parts motion) rather than a CSS keyframe, so it always runs to zero height.
 * Content fades out in the first half, then the empty row closes: never a cut-through slice.
 */
export function closeRow(node: HTMLElement, { height = 38 }: { height?: number } = {}) {
  node.style.overflow = 'hidden';
  return {
    duration: reducedMotion() ? 0 : 340,
    tick: (t: number) => {
      node.style.height = `${Math.min(1, t * 2) * height}px`;
      node.style.opacity = `${Math.max(0, 2 * t - 1)}`;
    },
  };
}

/** Svelte action: the split flash. A yellow overlay that burns off. */
export function flash(node: HTMLElement) {
  if (reducedMotion()) {
    node.style.opacity = '0';
    return;
  }
  animate(node, { opacity: [0.95, 0], duration: 1600, ease: 'outQuad' });
}

/** The cut: a hairline drops into the bar where a part was split in two. */
export function cut(node: HTMLElement) {
  if (reducedMotion()) return;
  animate(node, { scaleY: [0, 1], opacity: [1, 0], duration: 900, ease: 'outExpo' });
}
