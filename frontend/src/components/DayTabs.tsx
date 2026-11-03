import { For, type Component } from 'solid-js';

import { clock, dayShort, parseDate, todayIso } from '../lib/time';
import { cx } from '../lib/ui';

interface Props {
  days: { name: string; date: string }[];
  selected: string;
  onSelect: (date: string) => void;
  /** Ajoute le numéro du jour : `Mer 21`. */
  dated?: boolean;
}

/**
 * Sélecteur de jour (`.seg` de la maquette), quand l'écran ne tient qu'une
 * journée. Le jour du jour garde sa couleur, même quand un autre est ouvert.
 */
export const DayTabs: Component<Props> = (props) => (
  <div class="seg fill" role="tablist" aria-label="Jour de la semaine">
    <For each={props.days}>
      {(day) => (
        <button
          type="button"
          role="tab"
          class={cx(day.date === todayIso(clock()) && 'text-accent-hot!')}
          aria-selected={day.date === props.selected}
          onClick={() => props.onSelect(day.date)}
        >
          {props.dated ? `${dayShort(day.name)} ${parseDate(day.date).getDate()}` : dayShort(day.name)}
        </button>
      )}
    </For>
  </div>
);
