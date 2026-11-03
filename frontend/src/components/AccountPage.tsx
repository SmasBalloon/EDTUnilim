import { For, Show, createEffect, createResource, createSignal, type Component } from 'solid-js';
import { Bell, CalendarDays, FileText, LogOut } from 'lucide-solid';

import {
  ApiError,
  LOGIN_URL,
  deleteAccount,
  fetchMe,
  fetchWeek,
  logout,
  saveMe,
  type Delivery,
  type Me,
  type YearSummary,
} from '../lib/api';
import { groupBases, groupLabel } from '../lib/groups';
import { settings, update } from '../lib/settings';
import { resolveCurrentWeek } from '../lib/time';
import { button, chip, fieldRow, label, note, notice, option } from '../lib/ui';
import { GroupPicker } from './GroupPicker';
import { PageHead } from './Shell';

/** Motifs d'échec renvoyés par l'API au retour de Discord (`#compte?erreur=…`). */
const LOGIN_ERRORS: Record<string, string> = {
  annule: 'Connexion annulée.',
  session: 'La connexion a expiré ou a été interrompue — recommence.',
  discord: "Discord n'a pas pu confirmer la connexion. Réessaie dans un instant.",
};

const DELIVERY: Record<Delivery, string> = {
  none: 'Enregistré.',
  sent: 'Enregistré. Un message de confirmation t’attend sur Discord, avec l’emploi du temps de la semaine.',
  closed:
    'Enregistré, mais Discord refuse les messages privés : autorise les messages des membres du serveur dans tes paramètres de confidentialité.',
  failed: "Enregistré, mais le message de confirmation n'a pas pu partir. Les prochains seront retentés.",
};

/** Lit puis efface le motif d'échec de l'adresse, pour qu'il ne réapparaisse pas. */
function takeLoginError(): string | null {
  const code = new URLSearchParams(location.hash.split('?')[1] ?? '').get('erreur');
  if (code !== null) history.replaceState(null, '', '#compte');
  return code === null ? null : (LOGIN_ERRORS[code] ?? 'La connexion a échoué.');
}

/** Une carte de la page de compte (`compte.html`). */
const panel = 'card acct-card';

/**
 * Compte Discord : connexion, classe suivie et messages privés. La connexion
 * reste facultative — le site fonctionne sans.
 */
export const AccountPage: Component<{ years: YearSummary[] }> = (props) => {
  const loginError = takeLoginError();
  const [me, { mutate }] = createResource(fetchMe);

  const disabled = () => me.error instanceof ApiError && me.error.code === 'auth_disabled';

  return (
    <>
      <PageHead title="Compte" sub="Ta classe et tes messages privés sur Discord." />
      <div class="acct">
      <div class="flex w-full max-w-[560px] flex-col gap-4">
        <Show when={loginError}>
          {(message) => <p class={notice(true)}>{message()}</p>}
        </Show>

        <Show when={!me.loading} fallback={<p class={note}>Chargement…</p>}>
          <Show
            when={!me.error}
            fallback={
              <div class={panel}>
                <p class={note}>
                  {disabled()
                    ? "La connexion Discord n'est pas configurée sur ce serveur."
                    : `Compte indisponible : ${(me.error as Error).message}`}
                </p>
              </div>
            }
          >
            <Show when={me()} fallback={<SignIn />}>
              {(user) => <Profile me={user()} years={props.years} onChange={mutate} />}
            </Show>
          </Show>
        </Show>
      </div>
      </div>
    </>
  );
};

const SignIn: Component = () => (
  <section class={panel}>
    <h2>Reçois ton emploi du temps sur Discord</h2>
    <div class="flex flex-col gap-[18px]">
      <div class="acct-pt">
        <i>
          <CalendarDays size={22} aria-hidden="true" />
        </i>
        <p>
          Chaque nouvelle semaine publiée, en message privé, avec son fichier <b>.ics</b>.
        </p>
      </div>
      <div class="acct-pt">
        <i>
          <Bell size={22} aria-hidden="true" />
        </i>
        <p>Un message dès qu’un cours de ta classe change : salle, horaire, enseignant.</p>
      </div>
    </div>
    <p class="text-sm leading-normal text-muted">
      Tu seras ajouté au serveur Discord de l’emploi du temps : sans serveur commun, le bot ne peut pas t’écrire. Seuls
      ton identifiant, ton pseudo et ton avatar sont conservés.
    </p>
    <a class={`${button.discord} wide min-h-[52px]! text-base!`} href={LOGIN_URL}>
      Se connecter avec Discord
    </a>
  </section>
);

const Profile: Component<{ me: Me; years: YearSummary[]; onChange: (me: Me | null) => void }> = (props) => {
  // Brouillon : la classe du compte, sinon celle déjà choisie sur cet appareil.
  const [year, setYear] = createSignal(props.me.year ?? settings().year);
  const [group, setGroup] = createSignal(props.me.year === null ? settings().group : props.me.group);
  const [notify, setNotify] = createSignal(props.me.notify);
  const [status, setStatus] = createSignal<{ error: boolean; text: string } | null>(null);
  const [saving, setSaving] = createSignal(false);

  // Un compte déjà réglé sert de réglage à un appareil qui ne l'est pas encore.
  createEffect(() => {
    if (settings().year === null && props.me.year !== null && props.me.group !== null) {
      update({ year: props.me.year, group: props.me.group });
    }
  });

  const [bases] = createResource(year, async (chosen) => {
    const summary = props.years.find((item) => item.year === chosen);
    if (!summary || summary.weeks.length === 0) return [];
    return groupBases(await fetchWeek(chosen, await resolveCurrentWeek(chosen, summary)));
  });

  const dirty = () =>
    year() !== props.me.year || group()?.toLowerCase() !== props.me.group?.toLowerCase() || notify() !== props.me.notify;
  const complete = () => year() !== null && group() !== null;

  const save = async () => {
    setSaving(true);
    setStatus(null);
    try {
      const { user, dm } = await saveMe({ year: year(), group: group(), notify: notify() });
      props.onChange(user);
      if (user.year !== null && user.group !== null) update({ year: user.year, group: user.group });
      setStatus({ error: dm === 'closed' || dm === 'failed', text: DELIVERY[dm] });
    } catch (error) {
      setStatus({ error: true, text: `Enregistrement impossible : ${(error as Error).message}` });
    } finally {
      setSaving(false);
    }
  };

  const signOut = async () => {
    await logout().catch(() => undefined);
    props.onChange(null);
  };

  const remove = async () => {
    if (!confirm('Supprimer ton compte ? Tu ne recevras plus de messages privés.')) return;
    try {
      await deleteAccount();
      props.onChange(null);
    } catch (error) {
      setStatus({ error: true, text: `Suppression impossible : ${(error as Error).message}` });
    }
  };

  return (
    <>
      <section class={panel}>
        <header class="flex flex-wrap items-center gap-4">
          <Show
            when={props.me.avatar_url}
            fallback={
              <span class="grid size-12 shrink-0 place-items-center rounded-full bg-elevated font-display text-xl font-semibold text-muted uppercase">
                {props.me.name.charAt(0)}
              </span>
            }
          >
            {(url) => <img class="size-12 shrink-0 rounded-full object-cover" src={url()} alt="" width="48" height="48" />}
          </Show>
          <div class="min-w-0">
            <p class="font-display text-xl font-semibold">{props.me.name}</p>
            <p class={`${note} mt-0.5`}>
              {props.me.year !== null && props.me.group !== null
                ? `BUT ${props.me.year} ${groupLabel(props.me.group)}${props.me.notify ? ' · messages privés actifs' : ''}`
                : 'Aucune classe choisie'}
            </p>
          </div>
          <button type="button" class={`${button.plain} ml-auto`} onClick={signOut}>
            <LogOut size={18} aria-hidden="true" />
            Déconnexion
          </button>
        </header>
        <Show when={props.me.admin}>
          <a class={`${button.plain} wide`} href="#pdf">
            <FileText size={18} aria-hidden="true" />
            PDF en cache et dépôt manuel
          </a>
        </Show>
      </section>

      <section class={panel}>
        <h3>Ma classe</h3>

        <section class="sec">
          <h4 class={label}>Année</h4>
          <div class={fieldRow}>
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
        </section>

        <Show when={year() !== null}>
          <Show when={!bases.loading} fallback={<p class={note}>Lecture des groupes…</p>}>
            <GroupPicker bases={bases() ?? []} value={group()} onSelect={setGroup} />
          </Show>
        </Show>

        <section class="sec">
          <h4 class={label}>Messages privés</h4>
          <div class="grid gap-2.5 sm:grid-cols-2">
            <button type="button" class={option} aria-pressed={notify()} onClick={() => setNotify(true)}>
              <b>Activés</b>
              <span>Nouvelles semaines avec leur .ics, et changements de ta classe</span>
            </button>
            <button type="button" class={option} aria-pressed={!notify()} onClick={() => setNotify(false)}>
              <b>Désactivés</b>
              <span>Le compte garde ta classe, sans rien t’envoyer</span>
            </button>
          </div>
        </section>

        <Show when={status()}>
          {(current) => (
            <p class={notice(current().error)} role="status">
              {current().text}
            </p>
          )}
        </Show>

        <button
          type="button"
          class={`${button.primary} wide`}
          disabled={saving() || !complete() || !dirty()}
          onClick={save}
        >
          {saving() ? 'Enregistrement…' : 'Enregistrer'}
        </button>
      </section>

      <button
        type="button"
        class="cursor-pointer self-center px-2 py-1 text-[13px] text-muted underline underline-offset-3 hover:text-accent-ink"
        onClick={remove}
      >
        Supprimer mon compte
      </button>
    </>
  );
};
