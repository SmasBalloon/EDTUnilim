import { For, type Component } from 'solid-js';
import { X } from 'lucide-solid';

import {
  DEFAULT_ACCENT,
  settings,
  toAccent,
  update,
  type Theme,
  type ViewMode,
} from '../lib/settings';
import { GroupPicker } from './GroupPicker';
import { Sheet } from './Sheet';

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
  { value: 'grid', label: 'Grille horaire', hint: 'les cours placés sur une échelle 8 h – 19 h 30' },
  { value: 'list', label: 'Liste par jour', hint: 'les cours à la suite, avec leurs horaires écrits' },
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
    <header class="sheet__header">
      <h2 class="sheet__title">Réglages</h2>
      <button type="button" class="button" onClick={props.onClose} aria-label="Fermer">
        <X size={16} aria-hidden="true" />
      </button>
    </header>

    <section class="field">
      <h3 class="field__label">Affichage de la semaine</h3>
      <div class="field__stack">
        <For each={VIEWS}>
          {(view) => (
            <button
              type="button"
              class="option"
              aria-pressed={settings().view === view.value}
              onClick={() => update({ view: view.value })}
            >
              <span class="option__title">{view.label}</span>
              <span class="option__hint">{view.hint}</span>
            </button>
          )}
        </For>
      </div>
    </section>

    <section class="field">
      <h3 class="field__label">Apparence</h3>
      <div class="segment">
        <For each={THEMES}>
          {(theme) => (
            <button
              type="button"
              class="segment__option"
              aria-pressed={settings().theme === theme.value}
              onClick={() => update({ theme: theme.value })}
            >
              {theme.label}
            </button>
          )}
        </For>
      </div>
    </section>

    <section class="field">
      <h3 class="field__label">Couleur</h3>
      <div class="field__row picker-color">
        {/*
          * La pastille est le sélecteur lui-même : l'`input` couvre le disque
          * sans se voir, pour que le doigt ouvre la palette du système là où
          * la couleur se montre.
          */}
        <span class="picker-color__dot" style={{ '--swatch': accent() }}>
          <input
            type="color"
            class="picker-color__input"
            aria-label="Couleur d'accent"
            value={accent()}
            onInput={(event) => update({ accent: event.currentTarget.value })}
          />
        </span>
        <input
          type="text"
          class="picker-color__hex"
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
          class="chip"
          disabled={accent() === DEFAULT_ACCENT}
          onClick={() => update({ accent: DEFAULT_ACCENT })}
        >
          Rouge d'origine
        </button>
      </div>
    </section>

    <section class="field">
      <h3 class="field__label">Année</h3>
      <div class="field__row">
        <For each={props.years}>
          {(year) => (
            <button
              type="button"
              class="chip"
              aria-pressed={settings().year === year}
              onClick={() => update({ year, group: null })}
            >
              {`BUT ${year}`}
            </button>
          )}
        </For>
      </div>
    </section>

    <section class="field">
      <GroupPicker
        bases={props.bases}
        value={settings().group}
        onSelect={(group) => update({ group })}
      />
    </section>

    <div class="sheet__footer">
      <button
        type="button"
        class="button button--block"
        disabled={props.reloading}
        onClick={props.onReload}
      >
        {props.reloading ? 'Rechargement…' : 'Recharger les emplois du temps'}
      </button>
    </div>
  </Sheet>
);
