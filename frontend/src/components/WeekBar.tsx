import { Show, type Component } from 'solid-js';
import { ChevronLeft, ChevronRight } from 'lucide-solid';

import { humanDuration } from '../lib/time';
import { button, card, pill } from '../lib/ui';

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
    <div class={`${card} flex items-center gap-gap py-2 pr-2.5 pl-3.5`}>
      <div class="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1">
        <span class="text-[1.0625rem] font-semibold tracking-[-0.01em]">{`Semaine ${props.week ?? '—'}`}</span>
        <Show when={props.period}>
          {(label) => <span class="text-sm text-muted tabular-nums">{label()}</span>}
        </Show>
        <Show when={(props.workload ?? 0) > 0}>
          <span class={pill}>{humanDuration(props.workload ?? 0)}</span>
        </Show>
      </div>

      <div class="ml-auto flex shrink-0 items-center gap-1.5">
        <Show when={props.current !== undefined && props.week !== props.current}>
          <button type="button" class={button.plain} onClick={() => props.onWeek(props.current ?? 0)}>
            Aujourd'hui
          </button>
        </Show>
        <button
          type="button"
          class={button.round}
          onClick={() => step(-1)}
          disabled={position() <= 0}
          aria-label="Semaine précédente"
        >
          <ChevronLeft size={18} aria-hidden="true" />
        </button>
        <button
          type="button"
          class={button.round}
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
