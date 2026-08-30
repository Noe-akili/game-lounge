import { animate } from 'motion'
import gsap from 'gsap'

export function useAnimations() {
  const fadeIn = {
    initial: { opacity: 0, y: 12 },
    animate: { opacity: 1, y: 0 },
    transition: { duration: 0.3, ease: 'easeOut' as const },
  }

  const scaleIn = {
    initial: { opacity: 0, scale: 0.95 },
    animate: { opacity: 1, scale: 1 },
    transition: { duration: 0.25, ease: [0.22, 1, 0.36, 1] as const },
  }

  const stagger = (i: number) => ({
    initial: { opacity: 0, y: 10 },
    animate: { opacity: 1, y: 0 },
    transition: { duration: 0.25, delay: i * 0.04, ease: 'easeOut' as const },
  })

  const neonPulse = {
    animate: { opacity: [1, 0.6, 1], scale: [1, 1.02, 1] },
    transition: { duration: 1.5, repeat: Infinity, ease: 'easeInOut' as const },
  }

  const pulseGsap = (el: Element) => {
    gsap.to(el, { scale: 1.03, duration: 0.6, yoyo: true, repeat: -1, ease: 'power1.inOut' })
  }

  const hoverLift = {
    whileHover: { y: -4, scale: 1.01 },
    whileTap: { scale: 0.98 },
    transition: { duration: 0.2 },
  }

  return { fadeIn, scaleIn, stagger, neonPulse, pulseGsap, hoverLift, animate, gsap }
}
