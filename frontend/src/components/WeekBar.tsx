import { Show, type Component } from 'solid-js';
import { ChevronLeft, ChevronRight } from 'lucide-solid';

import { humanDuration } from '../lib/time';

interface Props {
  /** Semaine affichée, `null` tant qu'elle n'est pas connue. */
  week: number | null;
  /** Semaines publiées, dans l'ordre. */
  weeks: number[];
  /** Semaine contenant aujourd'hui, `undefined` tant qu'on la cherche. */
  current: number | undefined;
  /** `14 – 19 sept.` */
  period?: string;
  /** Volume horaire de la semaine, en minutes. */
  workload?: number;
  onWeek: (week: number) => void;
}

/** La semaine affichée, et de quoi en changer. */
export const WeekBar: Component<Props> = (props) => {
  const position = () => props.weeks.indexOf(props.week ?? -1);
  const step = (delta: number) => {
    const next = props.weeks[position() + delta];
    if (next !== undefined) props.onWeek(next);
  };

  return (
    <div class="card week-summary">
      <div class="week-summary__info">
        <span class="week-summary__number">{`Semaine ${props.week ?? '—'}`}</span>
        <Show when={props.period}>
          {(label) => <span class="week-summary__period">{label()}</span>}
        </Show>
        <Show when={(props.workload ?? 0) > 0}>
          <span class="week-summary__workload">{humanDuration(props.workload ?? 0)}</span>
        </Show>
      </div>

      <div class="week-summary__nav">
        <Show when={props.current !== undefined && props.week !== props.current}>
          <button type="button" class="button" onClick={() => props.onWeek(props.current ?? 0)}>
            Aujourd'hui
          </button>
        </Show>
        <button
          type="button"
          class="button button--round"
          onClick={() => step(-1)}
          disabled={position() <= 0}
          aria-label="Semaine précédente"
        >
          <ChevronLeft size={18} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="button button--round"
          onClick={() => step(1)}
          disabled={position() >= props.weeks.length - 1}
          aria-label="Semaine suivante"
        >
          <ChevronRight size={18} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
};
