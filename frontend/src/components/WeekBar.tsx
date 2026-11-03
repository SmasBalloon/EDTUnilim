import { Show, type Component } from 'solid-js';
import { ChevronLeft, ChevronRight } from 'lucide-solid';

import { humanDuration } from '../lib/time';
import { button } from '../lib/ui';

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

/** La semaine affichée, et de quoi en changer (`.wk` de la maquette). */
export const WeekBar: Component<Props> = (props) => {
  const position = () => props.weeks.indexOf(props.week ?? -1);
  const step = (delta: number) => {
    const next = props.weeks[position() + delta];
    if (next !== undefined) props.onWeek(next);
  };

  const detail = () =>
    [props.period, (props.workload ?? 0) > 0 ? humanDuration(props.workload ?? 0) : undefined]
      .filter(Boolean)
      .join(' · ');

  return (
    <div class="wk">
      <div class="flex min-w-0 flex-wrap items-baseline gap-x-3">
        <b class="whitespace-nowrap">{`Semaine ${props.week ?? '—'}`}</b>
        <Show when={detail()}>
          <span class="whitespace-nowrap tabular-nums">{detail()}</span>
        </Show>
      </div>
      <div class="flex shrink-0 items-center gap-1.5">
        <button
          type="button"
          class={button.plain}
          style={{ 'min-height': '40px' }}
          disabled={props.current === undefined || props.week === props.current}
          onClick={() => props.onWeek(props.current ?? 0)}
        >
          Aujourd'hui
        </button>
        <button
          type="button"
          class={button.icon}
          onClick={() => step(-1)}
          disabled={position() <= 0}
          aria-label="Semaine précédente"
        >
          <ChevronLeft size={22} aria-hidden="true" />
        </button>
        <button
          type="button"
          class={button.icon}
          onClick={() => step(1)}
          disabled={position() >= props.weeks.length - 1}
          aria-label="Semaine suivante"
        >
          <ChevronRight size={22} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
};
