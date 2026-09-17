import { For, Show, createEffect, createResource, createSignal, type Component } from 'solid-js';
import { LogOut } from 'lucide-solid';

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
import { GroupPicker } from './GroupPicker';

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

/**
 * Compte Discord : connexion, classe suivie et messages privés. La connexion
 * reste facultative — le site fonctionne sans.
 */
export const AccountPage: Component<{ years: YearSummary[] }> = (props) => {
  const loginError = takeLoginError();
  const [me, { mutate }] = createResource(fetchMe);

  const disabled = () => me.error instanceof ApiError && me.error.code === 'auth_disabled';

  return (
    <main class="page">
      <div class="account">
        <Show when={loginError}>
          {(message) => <p class="notice notice--error">{message()}</p>}
        </Show>

        <Show when={!me.loading} fallback={<p class="setup__note">Chargement…</p>}>
          <Show
            when={!me.error}
            fallback={
              <div class="card account__card">
                <p class="setup__note">
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
    </main>
  );
};

const SignIn: Component = () => (
  <section class="card account__card">
    <h2 class="account__title">Recevoir son emploi du temps sur Discord</h2>
    <ul class="account__list">
      <li>chaque nouvelle semaine publiée, en message privé, avec son fichier .ics ;</li>
      <li>un message dès qu’un cours de ta classe change : salle, horaire, enseignant.</li>
    </ul>
    <p class="setup__note">
      Tu seras ajouté au serveur Discord de l’emploi du temps : sans serveur commun, le bot ne peut pas
      t’écrire. Seuls ton identifiant, ton pseudo et ton avatar sont conservés.
    </p>
    <a class="button button--discord button--block" href={LOGIN_URL}>
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
      <section class="card account__card">
        <header class="account__profile">
          <Show
            when={props.me.avatar_url}
            fallback={<span class="account__avatar account__avatar--empty">{props.me.name.charAt(0)}</span>}
          >
            {(url) => <img class="account__avatar" src={url()} alt="" width="48" height="48" />}
          </Show>
          <div>
            <p class="account__name">{props.me.name}</p>
            <p class="setup__note">
              {props.me.year !== null && props.me.group !== null
                ? `BUT ${props.me.year} ${groupLabel(props.me.group)}${props.me.notify ? ' · messages privés actifs' : ''}`
                : 'Aucune classe choisie'}
            </p>
          </div>
          <button type="button" class="button account__logout" onClick={signOut}>
            <LogOut size={14} aria-hidden="true" />
            Déconnexion
          </button>
        </header>
      </section>

      <section class="card account__card">
        <h2 class="account__title">Ma classe</h2>

        <section class="field">
          <h3 class="field__label">Année</h3>
          <div class="field__row">
            <For each={props.years}>
              {(summary) => (
                <button
                  type="button"
                  class="chip"
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
          <section class="field">
            <Show when={!bases.loading} fallback={<p class="setup__note">Lecture des groupes…</p>}>
              <GroupPicker bases={bases() ?? []} value={group()} onSelect={setGroup} />
            </Show>
          </section>
        </Show>

        <section class="field">
          <h3 class="field__label">Messages privés</h3>
          <div class="field__stack">
            <button type="button" class="option" aria-pressed={notify()} onClick={() => setNotify(true)}>
              <span class="option__title">Activés</span>
              <span class="option__hint">nouvelles semaines avec leur .ics, et changements de ta classe</span>
            </button>
            <button type="button" class="option" aria-pressed={!notify()} onClick={() => setNotify(false)}>
              <span class="option__title">Désactivés</span>
              <span class="option__hint">le compte garde ta classe, sans rien t’envoyer</span>
            </button>
          </div>
        </section>

        <Show when={status()}>
          {(current) => (
            <p class="notice" classList={{ 'notice--error': current().error }} role="status">
              {current().text}
            </p>
          )}
        </Show>

        <button
          type="button"
          class="button button--primary button--block"
          disabled={saving() || !complete() || !dirty()}
          onClick={save}
        >
          {saving() ? 'Enregistrement…' : 'Enregistrer'}
        </button>
      </section>

      <button type="button" class="account__delete" onClick={remove}>
        Supprimer mon compte
      </button>
    </>
  );
};
