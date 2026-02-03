export interface Card {
  id: string;
  kind: { type: string; data?: string; };
}

export interface PlayerView {
  id: string;
  name: string;
  is_eliminated: boolean;
  hand_count: number; // New field
}

export interface GameState {
  phase: string | { [key: string]: any };
  deck_count: number; // New field
  discard_pile: Card[];
  players: PlayerView[];
  current_player_idx: number;
  my_hand: Card[]; // New field (replaces finding self in players array)
  logs: { timestamp: number; message: string }[];
  last_action_result?: string;
  // NEW: Server Timestamp
  last_move_ts: number;
}

const API_BASE = '/games';

export const api = {
  create: async () => {
    const res = await fetch(`${API_BASE}`, { method: 'POST' });
    if (!res.ok) throw new Error('Create failed');
    return res.json() as Promise<{ game_id: string }>;
  },

  join: async (gameId: string, name: string) => {
    const res = await fetch(`${API_BASE}/${gameId}/join`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ player_name: name }),
    });
    if (!res.ok) throw new Error('Join failed');
    return res.json() as Promise<{ player_id: string }>;
  },

  start: async (gameId: string) => {
    await fetch(`${API_BASE}/${gameId}/start`, { method: 'POST' });
  },

  getState: async (gameId: string, playerId: string) => {
    const res = await fetch(`${API_BASE}/${gameId}?player_id=${playerId}`);
    if (!res.ok) throw new Error('Poll failed');
    return res.json() as Promise<GameState>;
  },

  move: async (gameId: string, playerId: string, action: any) => {
    const res = await fetch(`${API_BASE}/${gameId}/move`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        player_id: playerId, // Security: Send who is acting
        action
      }),
    });

    if (!res.ok) {
      const txt = await res.text();
      throw new Error(txt);
    }
  },
};