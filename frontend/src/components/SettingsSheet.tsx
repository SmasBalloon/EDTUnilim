import { For, Show, type Component } from 'solid-js';
import { X } from 'lucide-solid';

import { settings, update, type Theme, type ViewMode } from '../lib/settings';
import { GroupPicker } from './GroupPicker';

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

export const SettingsSheet: Component<Props> = (props) => (
  <Show when={props.open}>
    <div class="sheet-backdrop" onClick={props.onClose}>
      <div class="sheet" onClick={(event) => event.stopPropagation()}>
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
      </div>
    </div>
  </Show>
);
