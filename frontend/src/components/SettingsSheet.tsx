import { For, Show, createSignal, type Component } from 'solid-js';
import { CalendarPlus, Check, Copy } from 'lucide-solid';

import { calendarUrl, webcalUrl } from '../lib/api';

import {
  DEFAULT_ACCENT,
  settings,
  toAccent,
  update,
  type Theme,
  type ViewMode,
} from '../lib/settings';
import { button, chipSmall, fieldRow, label, option, segment } from '../lib/ui';
import { GroupPicker } from './GroupPicker';
import { Sheet, SheetHeader } from './Sheet';

interface Props {
  open: boolean;
  /** Groupes principaux disponibles pour l'année suivie. */
  bases: string[];
  years: number[];
  /** Vrai pendant une relecture des emplois du temps. */
  reloading: boolean;
  onReload: () => void;
  onClose: () => void;
}

const VIEWS: { value: ViewMode; label: string; hint: string }[] = [
  { value: 'grid', label: 'Grille horaire', hint: 'Les cours posés sur une échelle 8 h – 20 h' },
  { value: 'list', label: 'Liste par jour', hint: 'Les cours à la suite, avec leurs horaires écrits' },
];

const THEMES: { value: Theme; label: string }[] = [
  { value: 'system', label: 'Système' },
  { value: 'light', label: 'Clair' },
  { value: 'dark', label: 'Sombre' },
];

/** La couleur retenue, ramenée à une forme que les champs savent afficher. */
const accent = () => toAccent(settings().accent) ?? DEFAULT_ACCENT;

export const SettingsSheet: Component<Props> = (props) => (
  <Sheet open={props.open} onClose={props.onClose} label="Réglages">
    <SheetHeader title="Réglages" onClose={props.onClose} />

    <div class="set-panel">
      <section class="sec">
        <h3 class={label}>Affichage de la semaine</h3>
        <div class="set-views">
          <For each={VIEWS}>
            {(view) => (
              <button
                type="button"
                class={option}
                aria-pressed={settings().view === view.value}
                onClick={() => update({ view: view.value })}
              >
                <b>{view.label}</b>
                <span>{view.hint}</span>
              </button>
            )}
          </For>
        </div>
      </section>

      <section class="sec">
        <h3 class={label}>Apparence</h3>
        <div class={`${segment} self-start`}>
          <For each={THEMES}>
            {(theme) => (
              <button
                type="button"
                aria-pressed={settings().theme === theme.value}
                onClick={() => update({ theme: theme.value })}
              >
                {theme.label}
              </button>
            )}
          </For>
        </div>
      </section>

      <section class="sec">
        <h3 class={label}>Couleur</h3>
        {/* Une pastille qui ouvre la palette du système, la valeur en clair à
            côté pour qui sait ce qu'il veut, et le retour au rouge d'origine. */}
        <div class={fieldRow}>
          {/*
            * La pastille est le sélecteur lui-même : l'`input` couvre le disque
            * sans se voir, pour que le doigt ouvre la palette du système là où
            * la couleur se montre.
            */}
          <span class="relative size-11 overflow-hidden rounded-full bg-accent">
            <input
              type="color"
              class="absolute inset-0 size-full cursor-pointer opacity-0"
              aria-label="Couleur d'accent"
              value={accent()}
              onInput={(event) => update({ accent: event.currentTarget.value })}
            />
          </span>
          <input
            type="text"
            class="input w-28 font-mono uppercase"
            inputMode="text"
            spellcheck={false}
            maxLength={7}
            aria-label="Couleur d'accent en hexadécimal"
            value={accent()}
            onInput={(event) => {
              const hex = toAccent(event.currentTarget.value);
              if (hex) update({ accent: hex });
            }}
            onBlur={(event) => {
              event.currentTarget.value = accent();
            }}
          />
          <button
            type="button"
            class={button.plain}
            disabled={accent() === DEFAULT_ACCENT}
            onClick={() => update({ accent: DEFAULT_ACCENT })}
          >
            Rouge d'origine
          </button>
        </div>
      </section>

      <div class="set-class">
        <section class="sec">
          <h3 class={label}>Année</h3>
          <div class="row gap-2">
            <For each={props.years}>
              {(year) => (
                <button
                  type="button"
                  class={chipSmall}
                  aria-pressed={settings().year === year}
                  aria-label={`BUT ${year}`}
                  onClick={() => update({ year, group: null })}
                >
                  {year}
                </button>
              )}
            </For>
          </div>
        </section>

        <GroupPicker
          inline
          baseLabel="Groupe"
          bases={props.bases}
          value={settings().group}
          onSelect={(group) => update({ group })}
        />
      </div>

      <Show when={settings().year !== null && settings().group !== null}>
        <Subscribe year={settings().year ?? 0} group={settings().group ?? ''} />
      </Show>

      <button
        type="button"
        class={`${button.plain} wide`}
        disabled={props.reloading}
        onClick={props.onReload}
      >
        {props.reloading ? 'Rechargement…' : 'Recharger les emplois du temps'}
      </button>
    </div>
  </Sheet>
);

/**
 * L'abonnement de l'agenda : toutes les semaines publiées de la classe, que
 * l'agenda relit de lui-même. Le clic ouvre l'agenda du système (`webcal:`) ;
 * Google Agenda, lui, demande qu'on colle l'adresse.
 */
const Subscribe: Component<{ year: number; group: string }> = (props) => {
  const [copied, setCopied] = createSignal(false);

  const copy = async () => {
    const url = calendarUrl(props.year, props.group);
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Presse-papiers refusé (page non sécurisée, permission) : l'adresse
      // reste lisible, à copier à la main.
      window.prompt('Copie cette adresse :', url);
    }
  };

  return (
    <section class="sec">
      <h3 class={label}>Calendrier</h3>
      <div class={fieldRow}>
        <a class={button.primary} href={webcalUrl(props.year, props.group)}>
          <CalendarPlus size={20} aria-hidden="true" />
          S'abonner
        </a>
        <button type="button" class={button.plain} onClick={() => void copy()}>
          <Show when={copied()} fallback={<Copy size={20} aria-hidden="true" />}>
            <Check size={20} aria-hidden="true" />
          </Show>
          {copied() ? 'Adresse copiée' : 'Copier l’adresse'}
        </button>
      </div>
      <p class="text-[13px] leading-normal text-muted">
        {`Les cours de ${props.group.toUpperCase()} dans ton agenda, mis à jour tout seuls. Pour Google Agenda : copie l’adresse, puis Autres agendas → À partir de l’URL.`}
      </p>
    </section>
  );
};
