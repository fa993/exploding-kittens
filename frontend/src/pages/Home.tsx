import { useState } from 'preact/hooks';
import { route } from 'preact-router';
import { api } from '../api';
import { base } from '../utils';

interface HomeProps { path?: string; }

export function Home(_props: HomeProps) {
  const [name, setName] = useState('');
  const [joinId, setJoinId] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(''); // Store validation errors

  const savePlayer = (id: string, name: string) => {
    sessionStorage.setItem('player_id', id);
    sessionStorage.setItem('player_name', name);
  };

  const createGame = async () => {
    if (!name.trim()) {
      setError('Please enter your name first!');
      return;
    }

    setError('');
    setLoading(true);
    try {
      const { game_id } = await api.create();
      const { player_id } = await api.join(game_id, name);
      savePlayer(player_id, name);
      route(`${base}/game/${game_id}`);
    } catch (e) {
      setError('Failed to create game. Server might be down.');
    } finally {
      setLoading(false);
    }
  };

  const joinGame = async () => {
    if (!name.trim()) {
      setError('Please enter your name first!');
      return;
    }
    if (!joinId.trim()) {
      setError('Please enter a valid Game ID!');
      return;
    }

    setError('');
    setLoading(true);
    try {
      const { player_id } = await api.join(joinId, name);
      savePlayer(player_id, name);
      route(`${base}/game/${joinId}`);
    } catch (e) {
      setError('Could not join. Check the Game ID.');
    } finally {
      setLoading(false);
    }
  };

  // Clear error when user types
  const handleInput = (setter: (v: string) => void, value: string) => {
    setter(value);
    if (error) setError('');
  };

  return (
    <div className="container">
      <div style={{ textAlign: 'center', marginBottom: 40 }}>
        <h1 style={{ fontSize: '3rem', margin: 0, color: '#ef4444' }}>💣 KITTENS</h1>
        <p style={{ color: '#94a3b8' }}>An explosive card game</p>
      </div>

      <div style={{ background: '#1e293b', padding: 30, borderRadius: 16, width: 350, boxShadow: '0 20px 25px -5px rgba(0, 0, 0, 0.1)' }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 15 }}>

          {/* Error Message Display */}
          {error && (
            <div style={{
              background: 'rgba(239, 68, 68, 0.1)',
              color: '#ef4444',
              padding: '10px',
              borderRadius: '8px',
              fontSize: '0.9rem',
              textAlign: 'center',
              border: '1px solid rgba(239, 68, 68, 0.2)'
            }}>
              {error}
            </div>
          )}

          <label style={{ fontWeight: 600, fontSize: '0.9rem', color: '#cbd5e1' }}>Player Name</label>
          <input
            value={name}
            onInput={(e) => handleInput(setName, e.currentTarget.value)}
            style={{ padding: 12, borderRadius: 8, border: '1px solid #334155', background: '#0f172a', color: 'white' }}
            placeholder="Enter your name"
          />

          <button
            onClick={createGame}
            disabled={loading}
            style={{ background: '#ef4444', color: 'white', padding: 14, borderRadius: 8, marginTop: 10 }}
          >
            {loading ? 'Creating...' : 'Create New Game'}
          </button>

          <div style={{ display: 'flex', alignItems: 'center', gap: 10, margin: '10px 0' }}>
            <hr style={{ flex: 1, borderColor: '#334155' }} /> <span style={{ fontSize: '0.8rem', color: '#64748b' }}>OR</span> <hr style={{ flex: 1, borderColor: '#334155' }} />
          </div>

          <label style={{ fontWeight: 600, fontSize: '0.9rem', color: '#cbd5e1' }}>Join by ID</label>
          <div style={{ display: 'flex', gap: 10 }}>
            <input
              value={joinId}
              onInput={(e) => handleInput(setJoinId, e.currentTarget.value)}
              style={{ flex: 1, padding: 12, borderRadius: 8, border: '1px solid #334155', background: '#0f172a', color: 'white' }}
              placeholder="Game UUID"
            />
            <button
              onClick={joinGame}
              disabled={loading}
              style={{ background: '#3b82f6', color: 'white', padding: '0 20px', borderRadius: 8 }}
            >
              {loading ? '...' : 'Join'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}