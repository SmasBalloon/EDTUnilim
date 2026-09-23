import { For, type Component } from 'solid-js';

import { clock, parseDate, todayIso } from '../lib/time';
import { cx, press } from '../lib/ui';

interface Props {
  days: { name: string; date: string }[];
  selected: string;
  onSelect: (date: string) => void;
}

/**
 * Sélecteur de jour, affiché quand l'écran ne tient qu'une journée.
 *
 * Les jours forment un seul objet, posé dans une gouttière : on y lit la
 * semaine entière, pas six boutons alignés.
 */
export const DayTabs: Component<Props> = (props) => (
  <div
    class="mx-edge mb-3.5 flex gap-[0.1875rem] rounded-[calc(var(--radius-control)+0.1875rem)] bg-elevated p-[0.1875rem]"
    role="tablist"
    aria-label="Jour de la semaine"
  >
    <For each={props.days}>
      {(day) => {
        const date = parseDate(day.date);
        return (
          <button
            type="button"
            role="tab"
            class={cx(
              press,
              'relative grid flex-1 cursor-pointer justify-items-center gap-px rounded-control px-1 pt-[0.3125rem] pb-[0.4375rem]',
              // Le jour du jour reste repérable, même quand un autre est ouvert :
              // un point suffit, la couleur pleine est réservée à la sélection.
              day.date === todayIso(clock())
                ? 'text-accent after:absolute after:bottom-[0.1875rem] after:size-1 after:rounded-full after:bg-current'
                : 'text-muted',
              'aria-selected:bg-accent aria-selected:text-on-accent aria-selected:shadow-card',
            )}
            aria-selected={day.date === props.selected}
            onClick={() => props.onSelect(day.date)}
          >
            <span class="text-[0.625rem] font-semibold uppercase tracking-[0.06em]">{day.name.slice(0, 3)}</span>
            <span class="text-[0.9375rem] font-semibold tabular-nums">{date.getDate()}</span>
          </button>
        );
      }}
    </For>
  </div>
);
