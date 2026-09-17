import { For, Show, createResource, createSignal, type Component } from 'solid-js';

import type { YearSummary } from '../lib/api';
import { fetchWeek } from '../lib/api';
import { groupBases } from '../lib/groups';
import { update } from '../lib/settings';
import { resolveCurrentWeek } from '../lib/time';
import { GroupPicker } from './GroupPicker';

/**
 * Premier lancement : l'année puis le groupe, une bonne fois pour toutes. Les
 * groupes proposés sont lus dans la semaine en cours.
 */
export const Setup: Component<{ years: YearSummary[] }> = (props) => {
  const [year, setYear] = createSignal<number | null>(null);

  const [bases] = createResource(year, async (chosen) => {
    const summary = props.years.find((item) => item.year === chosen);
    if (!summary || summary.weeks.length === 0) return [];
    const week = await resolveCurrentWeek(chosen, summary);
    return groupBases(await fetchWeek(chosen, week));
  });

  return (
    <main class="setup">
      <div class="setup__panel">
        <h1 class="setup__title">Emploi du temps</h1>
        <p class="setup__intro">
          IUT du Limousin — département informatique. Choisis ton année et ton groupe : ils sont
          gardés sur cet appareil.
        </p>

        <section class="field">
          <h2 class="field__label">Année</h2>
          <div class="field__row">
            <For each={props.years}>
              {(summary) => (
                <button
                  type="button"
                  class="chip"
                  aria-pressed={year() === summary.year}
                  onClick={() => setYear(summary.year)}
                >
                  {`BUT ${summary.year}`}
                </button>
              )}
            </For>
          </div>
        </section>

        <p class="setup__note">
          Ou <a class="setup__link" href="#compte">connecte-toi avec Discord</a> pour recevoir ton emploi du
          temps et ses changements en message privé.
        </p>

        <Show when={year() !== null}>
          <section class="field">
            <Show when={!bases.loading} fallback={<p class="setup__note">Lecture des groupes…</p>}>
              <Show
                when={!bases.error}
                fallback={<p class="setup__note">Emploi du temps indisponible pour le moment.</p>}
              >
                <GroupPicker
                  bases={bases() ?? []}
                  value={null}
                  onSelect={(group) => update({ year: year(), group })}
                />
              </Show>
            </Show>
          </section>
        </Show>
      </div>
    </main>
  );
};
