//! Ressorts et élan : la matière dont sont faits les gestes.
//!
//! Une animation à durée fixe ne sait pas répondre à un doigt qui change
//! d'avis. Un ressort, si : on lui donne une nouvelle cible et il repart de
//! là où il est, à la vitesse qu'il a. Tout ce qui se touche passe par ici.

/** Vrai quand le système demande à ce que ça bouge le moins possible. */
export function prefersReducedMotion(): boolean {
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

export interface SpringOptions {
  /**
   * Dépassement : `1` s'arrête net sur la cible, en dessous ça rebondit. On ne
   * rebondit que lorsque le geste, lui, portait de l'élan.
   */
  damping?: number;
  /** Vivacité, en secondes. Ce n'est pas une durée : le ressort n'en a pas. */
  response?: number;
  /** Vitesse initiale, en unités par seconde — celle du doigt au lâcher. */
  velocity?: number;
}

/** Pas d'intégration : assez fin pour rester stable à ressort raide. */
const STEP = 1 / 240;

/**
 * Un ressort amorti sur une valeur, intégré à chaque image.
 *
 * Interruptible par construction : `to()` ne coupe rien, il change la cible ;
 * la position et la vitesse courantes sont conservées, donc aucun saut.
 */
export class Spring {
  value: number;
  velocity = 0;
  target: number;

  private damping = 1;
  private response = 0.4;
  private frame = 0;
  private last = 0;

  constructor(
    value: number,
    private readonly onFrame: (value: number) => void,
    /** Distance en deçà de laquelle on considère la valeur posée. */
    private readonly epsilon = 0.002,
  ) {
    this.value = value;
    this.target = value;
  }

  /** Pose la valeur sans mouvement : le geste tient l'objet lui-même. */
  set(value: number): void {
    this.stop();
    this.value = value;
    this.target = value;
    this.velocity = 0;
    this.onFrame(value);
  }

  /** Vise une nouvelle cible, depuis l'état courant. */
  to(target: number, options: SpringOptions = {}): void {
    this.target = target;
    this.damping = options.damping ?? 1;
    this.response = options.response ?? 0.4;
    if (options.velocity !== undefined) this.velocity = options.velocity;

    if (prefersReducedMotion()) {
      this.set(target);
      return;
    }
    if (this.frame === 0) {
      this.last = performance.now();
      this.frame = requestAnimationFrame(this.tick);
    }
  }

  stop(): void {
    if (this.frame !== 0) cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.velocity = 0;
  }

  private readonly tick = (now: number): void => {
    // Une image perdue ne doit pas téléporter l'objet : on plafonne le temps
    // écoulé, quitte à prendre un peu de retard sur la simulation.
    const elapsed = Math.min(0.064, (now - this.last) / 1000);
    this.last = now;

    const omega = (2 * Math.PI) / this.response;
    const stiffness = omega * omega;
    const friction = 2 * this.damping * omega;

    for (let left = elapsed; left > 0; left -= STEP) {
      const dt = Math.min(STEP, left);
      const force = stiffness * (this.target - this.value) - friction * this.velocity;
      this.velocity += force * dt;
      this.value += this.velocity * dt;
    }

    if (
      Math.abs(this.target - this.value) < this.epsilon &&
      Math.abs(this.velocity) < this.epsilon
    ) {
      this.frame = 0;
      this.value = this.target;
      this.velocity = 0;
      this.onFrame(this.value);
      return;
    }

    this.onFrame(this.value);
    this.frame = requestAnimationFrame(this.tick);
  };
}

/**
 * Où s'arrêterait un objet lancé à cette vitesse, s'il décélérait seul.
 *
 * C'est la projection du défilement : on choisit la destination d'après le
 * point visé par le geste, pas d'après celui où le doigt s'est levé.
 */
export function project(velocity: number, deceleration = 0.998): number {
  return ((velocity / 1000) * deceleration) / (1 - deceleration);
}

/**
 * Résistance élastique au-delà d'une limite : plus on tire, moins ça suit.
 * Un arrêt net se lit comme un blocage ; une résistance se lit comme un bord.
 */
export function rubberband(overshoot: number, dimension: number, constant = 0.55): number {
  return (overshoot * dimension * constant) / (dimension + constant * Math.abs(overshoot));
}

/** Relevé de position destiné au calcul de la vitesse au lâcher. */
export class Velocity {
  private samples: { value: number; at: number }[] = [];

  add(value: number, at: number = performance.now()): void {
    this.samples.push({ value, at });
    if (this.samples.length > 6) this.samples.shift();
  }

  clear(): void {
    this.samples = [];
  }

  /** Vitesse en unités par seconde, lue sur les dernières images. */
  get(): number {
    const last = this.samples[this.samples.length - 1];
    // On remonte d'environ 80 ms : assez pour lisser, assez court pour que la
    // vitesse soit encore celle du geste et non sa moyenne.
    const first = this.samples.find((sample) => last !== undefined && last.at - sample.at < 80);
    if (!last || !first || last.at === first.at) return 0;
    return ((last.value - first.value) / (last.at - first.at)) * 1000;
  }
}
