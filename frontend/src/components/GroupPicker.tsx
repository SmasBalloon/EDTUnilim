import { For, Show, createSignal, type Component } from 'solid-js';

import { HALVES, baseOf, groupLabel, halfOf } from '../lib/groups';
import { chip, chipSmall, fieldRow, label, note } from '../lib/ui';

interface Props {
  /** Groupes principaux disponibles : `G1`, `G2`, … */
  bases: string[];
  /** Demi-groupe retenu, `G1a`, ou `null` si rien n'est encore choisi. */
  value: string | null;
  onSelect: (group: string) => void;
  /**
   * Les deux choix se posent dans la grille du parent (réglages : Année,
   * Groupe, Sous-groupe sur une ligne) au lieu de s'empiler.
   */
  inline?: boolean;
  /** Libellé du groupe principal : « Groupe principal » ou « Groupe ». */
  baseLabel?: string;
}

/**
 * Deux choix successifs : le groupe principal, puis la moitié.
 *
 * On désigne toujours un demi-groupe : demander `G1a` rend aussi les cours du
 * groupe entier, alors que `G1` ferait apparaître les TP de l'autre moitié.
 *
 * Les groupes viennent de l'emploi du temps lui-même — l'API n'en publie pas la
 * liste, ils se lisent dans les créneaux.
 */
export const GroupPicker: Component<Props> = (props) => {
  // Le groupe principal peut être choisi avant sa moitié : il vit ici tant que
  // la sélection n'est pas complète.
  const [pending, setPending] = createSignal<string | null>(null);

  const base = () => pending() ?? (props.value === null ? null : baseOf(props.value));
  const half = () => (props.value === null ? null : halfOf(props.value));

  const chooseBase = (chosen: string) => {
    setPending(chosen);
    // Changer de groupe en gardant sa moitié : G1A → G2A.
    const current = half();
    if (current !== null) props.onSelect(`${chosen}${current}`);
  };

  const chooseHalf = (chosen: string) => {
    const group = base();
    if (group !== null) props.onSelect(`${group}${chosen}`);
  };

  return (
    <Show
      when={props.bases.length > 0}
      fallback={
        <p class={note}>
          Aucun groupe dans cette semaine — essaie une autre semaine ou une autre année.
        </p>
      }
    >
      <div class={props.inline ? 'contents' : 'flex flex-col gap-7'}>
        <div class="flex flex-col gap-3">
          <span class={label}>{props.baseLabel ?? 'Groupe principal'}</span>
          <div class={fieldRow}>
            <For each={props.bases}>
              {(item) => (
                <button
                  type="button"
                  class={props.inline ? chipSmall : chip}
                  aria-pressed={base() === item}
                  onClick={() => chooseBase(item)}
                >
                  {groupLabel(item)}
                </button>
              )}
            </For>
          </div>
        </div>

        <div class="flex flex-col gap-3">
          <span class={label}>Sous-groupe</span>
          <div class={fieldRow}>
            <For each={HALVES}>
              {(item) => (
                <button
                  type="button"
                  class={props.inline ? chipSmall : chip}
                  disabled={base() === null}
                  aria-pressed={half() === item}
                  onClick={() => chooseHalf(item)}
                >
                  {item.toUpperCase()}
                </button>
              )}
            </For>
          </div>
        </div>
      </div>
    </Show>
  );
};
