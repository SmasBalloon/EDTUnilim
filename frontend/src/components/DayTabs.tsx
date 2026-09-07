import { For, type Component } from 'solid-js';

import { clock, parseDate, todayIso } from '../lib/time';

interface Props {
  days: { name: string; date: string }[];
  selected: string;
  onSelect: (date: string) => void;
}

/** Sélecteur de jour, affiché quand l'écran ne tient qu'une journée. */
export const DayTabs: Component<Props> = (props) => (
  <div class="day-tabs" role="tablist" aria-label="Jour de la semaine">
    <For each={props.days}>
      {(day) => {
        const date = parseDate(day.date);
        return (
          <button
            type="button"
            role="tab"
            class="day-tab"
            classList={{ 'day-tab--today': day.date === todayIso(clock()) }}
            aria-selected={day.date === props.selected}
            onClick={() => props.onSelect(day.date)}
          >
            <span class="day-tab__name">{day.name.slice(0, 3)}</span>
            <span class="day-tab__date">{date.getDate()}</span>
          </button>
        );
      }}
    </For>
  </div>
);
