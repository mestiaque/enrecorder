/** Which library item the Media Editor opens (set from the Recordings detail panel). */
import { create } from "zustand";

export const useEditorTarget = create<{ id: string | null; open(id: string | null): void }>((set) => ({ id: null, open: (id) => set({ id }) }));
