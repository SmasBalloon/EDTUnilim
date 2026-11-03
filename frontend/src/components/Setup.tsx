import { For, Show, createMemo, createResource, createSignal, type Component } from 'solid-js';

import type { Course, YearSummary } from '../lib/api';
import { fetchWeek } from '../lib/api';
import { groupBases } from '../lib/groups';
import { KIND_LABEL } from '../lib/layout';
import { update } from '../lib/settings';
import { clock, nowMinutes, relativeDayLabel, resolveCurrentWeek, toMinutes, todayIso } from '../lib/time';
import { button, chip, kindTag, label, link, note } from '../lib/ui';
import { GroupPicker } from './GroupPicker';

/** Le prochain cours d'une semaine, après maintenant. */
function nextCourse(courses: Course[]): Course | null {
  const today = todayIso(clock());
  const minutes = nowMinutes(clock());
  return (
    courses
      .filter((course) => course.date > today || (course.date === today && toMinutes(course.start) > minutes))
      .sort((a, b) => a.date.localeCompare(b.date) || toMinutes(a.start) - toMinutes(b.start))[0] ?? null
  );
}

/**
 * Premier lancement (`accueil.html`) : l'année, le groupe et le sous-groupe,
 * confirmés d'un bouton. Les groupes proposés sont lus dans la semaine en
 * cours ; dès que la classe est complète, l'aperçu montre son prochain cours.
 */
export const Setup: Component<{ years: YearSummary[] }> = (props) => {
  const [year, setYear] = createSignal<number | null>(null);
  const [group, setGroup] = createSignal<string | null>(null);

  const [week] = createResource(year, async (chosen) => {
    const summary = props.years.find((item) => item.year === chosen);
    if (!summary || summary.weeks.length === 0) return null;
    const current = await resolveCurrentWeek(chosen, summary);
    return { number: current, structure: await fetchWeek(chosen, current) };
  });

  const bases = () => {
    const loaded = week();
    return loaded ? groupBases(loaded.structure) : [];
  };

  /** Le prochain cours de la classe choisie, pour l'aperçu. */
  const [preview] = createResource(
    () => {
      const chosen = year();
      const half = group();
      const loaded = week();
      return chosen !== null && half !== null && loaded ? { year: chosen, group: half, week: loaded.number } : undefined;
    },
    async ({ year: chosen, group: half, week: number }) => nextCourse((await fetchWeek(chosen, number, half)).courses),
  );

  const when = createMemo(() => {
    const course = preview();
    if (!course) return '';
    const [position] = relativeDayLabel(course.date, clock()).split(' : ');
    return course.date === todayIso(clock()) ? 'aujourd’hui' : (position ?? '').toLowerCase();
  });

  const confirm = () => {
    const chosen = year();
    const half = group();
    if (chosen !== null && half !== null) update({ year: chosen, group: half });
  };

  return (
    <div class="setup">
      <div class="setup-intro">
        <div class="row gap-3.5">
          <div class="logo m-0!">EDT</div>
          <span class={label}>IUT du Limousin · Informatique</span>
        </div>

        <div class="mt-16 flex max-w-[560px] flex-col gap-5 max-[900px]:hidden">
          <h1>Ton emploi du temps, sans ouvrir le PDF.</h1>
          <p class="sub text-lg leading-normal">
            Choisis ta classe une fois. Ta semaine, tes salles et tes changements sont toujours à portée de pouce.
          </p>
        </div>

        <Show when={preview()}>
          {(course) => (
            <div class="card mt-auto flex max-w-[520px] flex-col gap-3.5 p-5 max-[900px]:hidden">
              <span class={label}>{`Aperçu · ${when()}`}</span>
              <div class="row justify-between">
                <div>
                  <b class="text-[17px]">{course().resource?.code ?? course().title ?? 'Cours'}</b>
                  <p class="mt-0.5 text-sm text-muted">
                    {[`${course().start} – ${course().end}`, course().resource?.name ?? course().teacher]
                      .filter(Boolean)
                      .join(' · ')}
                  </p>
                </div>
                <div class="row gap-2">
                  <span class={kindTag(course().kind)}>{KIND_LABEL[course().kind]}</span>
                  <Show when={course().room}>{(room) => <span class="tag t-room">{room()}</span>}</Show>
                </div>
              </div>
            </div>
          )}
        </Show>
      </div>

      <div class="setup-form">
        <div>
          <h2 class="max-[900px]:hidden">Choisis ta classe</h2>
          <h1 class="hidden font-display text-[34px] leading-tight font-semibold max-[900px]:block">Choisis ta classe</h1>
          <p class="sub">Ton année et ton groupe sont gardés sur cet appareil.</p>
        </div>

        <div class="flex flex-col gap-3">
          <span class={label}>Année</span>
          <div class="row gap-2.5">
            <For each={props.years}>
              {(summary) => (
                <button
                  type="button"
                  class={chip}
                  aria-pressed={year() === summary.year}
                  onClick={() => {
                    if (year() !== summary.year) setGroup(null);
                    setYear(summary.year);
                  }}
                >
                  {`BUT ${summary.year}`}
                </button>
              )}
            </For>
          </div>
        </div>

        <Show when={year() !== null}>
          <Show when={!week.loading} fallback={<p class={note}>Lecture des groupes…</p>}>
            <Show when={!week.error} fallback={<p class={note}>Emploi du temps indisponible pour le moment.</p>}>
              <GroupPicker bases={bases()} value={group()} onSelect={setGroup} />
            </Show>
          </Show>
        </Show>

        <button
          type="button"
          class={`${button.primary} wide min-h-[52px]! text-base!`}
          disabled={year() === null || group() === null}
          onClick={confirm}
        >
          Voir mon emploi du temps
        </button>

        <div class="setup-links">
          <p>
            Ou <a class={link} href="#compte">connecte-toi avec Discord</a> pour recevoir ton emploi du temps et ses
            changements en message privé.
          </p>
          {/* L'analyse ne dépend d'aucune classe : on y va sans en choisir une. */}
          <p>
            Ou <a class={link} href="#analyse">analyse des PDF</a> pour repérer les salles réservées deux fois, sans
            choisir de groupe.
          </p>
        </div>
      </div>
    </div>
  );
};
