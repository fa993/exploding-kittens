import { useEffect, useState, useRef } from 'preact/hooks';
import { route } from 'preact-router';
import { api, GameState } from '../api';

interface GameBoardProps { gameId?: string; path?: string; }

type ActionType = 'None' | 'Simple' | 'Target' | 'Pair' | 'Defuse';

export function GameBoard({ gameId }: GameBoardProps) {
  if (!gameId) return <div className="container">Error: No Game ID</div>;

  const [state, setState] = useState<GameState | null>(null);
  const [selected, setSelected] = useState<number[]>([]);
  const [toasts, setToasts] = useState<{ msg: string, type: 'info' | 'error', id: number }[]>([]);

  // UI States
  const [targetMode, setTargetMode] = useState<'Favor' | 'Pair' | null>(null);
  const [showLogs, setShowLogs] = useState(false);
  const [isActionPending, setIsActionPending] = useState(false);

  const logsEndRef = useRef<HTMLDivElement>(null);
  const playerId = sessionStorage.getItem('player_id') || '';

  // --- POLLING ---
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const s = await api.getState(gameId, playerId);
        setState(s);
      } catch (e) { console.error("Poll fail", e); }
    }, 1000);
    return () => clearInterval(interval);
  }, [gameId]);

  // Auto Scroll Logs
  useEffect(() => {
    if (showLogs && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [state?.logs, showLogs]);

  const showToast = (msg: string, type: 'info' | 'error' = 'info') => {
    const id = Date.now();
    setToasts(t => [...t, { msg, type, id }]);
    setTimeout(() => setToasts(t => t.filter(x => x.id !== id)), 3000);
  };

  const handleExit = () => {
    if (confirm("Are you sure you want to leave the game?")) route('/');
  };

  // NEW: Copy to Clipboard Handler
  const copyGameId = async () => {
    try {
      await navigator.clipboard.writeText(gameId);
      showToast("Game ID copied to clipboard!");
    } catch (err) {
      showToast("Failed to copy ID", "error");
    }
  };

  if (!state) return <div className="container">Loading Game...</div>;

  // --- STATE HELPERS ---
  const myIdx = state.players.findIndex(p => p.id === playerId);
  const isMyTurn = state.current_player_idx === myIdx;
  const phaseStr = typeof state.phase === 'string' ? state.phase : Object.keys(state.phase)[0];
  const isExploding = phaseStr === 'ExplosionPending';

  const myHand = state.my_hand || [];
  const isEliminated = state.players[myIdx]?.is_eliminated;
  const hasDefuse = myHand.some(c => c.kind.type === 'Defuse');

  // --- SMART ACTION DETECTION ---
  let actionType: ActionType = 'None';
  let actionLabel = "Select Cards";

  if (selected.length === 1) {
    const card = myHand[selected[0]];
    const type = card.kind.type;
    if (['Attack', 'Skip', 'Shuffle', 'See the Future'].includes(type)) {
      actionType = 'Simple';
      actionLabel = `Play ${type}`;
    } else if (type === 'Favor') {
      actionType = 'Target';
      actionLabel = "Play Favor (Select Target)";
    }
  } else if (selected.length === 2) {
    const c1 = myHand[selected[0]];
    const c2 = myHand[selected[1]];
    if (c1.kind.type === c2.kind.type) {
      actionType = 'Pair';
      actionLabel = `Play Pair (Steal)`;
    }
  }

  // --- HANDLERS ---
  const handleDraw = async () => {
    if (isActionPending) return;
    setIsActionPending(true);
    try { await api.move(gameId, playerId, { event: 'DrawCard' }); }
    catch (e: any) { showToast(e.message, 'error'); }
    finally { setIsActionPending(false); }
  };

  const executeAction = async () => {
    if (isActionPending) return;
    setIsActionPending(true);
    try {
      if (actionType === 'Simple') {
        const idx = selected[0];
        const rawType = myHand[idx].kind.type;

        let eventName = "";
        if (rawType === "See the Future") eventName = "PlaySeeTheFuture";
        else eventName = `Play${rawType.replace(/\s/g, '')}`;

        await api.move(gameId, playerId, { event: eventName, data: { card_idx: idx } });
        setSelected([]);
      }
      else if (actionType === 'Target' || actionType === 'Pair') {
        setTargetMode(actionType === 'Target' ? 'Favor' : 'Pair');
        showToast("Click an opponent to target them!");
      }
    } catch (e: any) { showToast(e.message, 'error'); }
    finally { setIsActionPending(false); }
  };

  const handleTargetClick = async (targetIdx: number) => {
    if (!targetMode || isActionPending) return;
    if (targetIdx === myIdx) return showToast("Can't target yourself!", 'error');

    setIsActionPending(true);
    try {
      if (targetMode === 'Favor') {
        await api.move(gameId, playerId, { event: 'PlayFavor', data: { card_idx: selected[0], target_idx: targetIdx } });
      } else if (targetMode === 'Pair') {
        const sorted = [...selected].sort((a, b) => a - b);
        await api.move(gameId, playerId, { event: 'PlayPair', data: { card_indices: sorted, target_idx: targetIdx } });
      }
      setSelected([]);
      setTargetMode(null);
    } catch (e: any) { showToast(e.message, 'error'); }
    finally { setIsActionPending(false); }
  };

  const handleDefuse = async () => {
    if (isActionPending) return;
    const defuseIdx = myHand.findIndex(c => c.kind.type === 'Defuse');
    if (defuseIdx === -1) return showToast("No Defuse Card!", 'error');

    setIsActionPending(true);
    try {
      // 2. CALCULATE RANDOM DEPTH ON CLIENT
      // Range: 0 (Bottom) to deck_count (Top)
      // We use state.deck_count from the view we added earlier
      const maxDepth = state.deck_count;
      const randomDepth = Math.floor(Math.random() * (maxDepth + 1));

      // 3. Send to Backe
      await api.move(gameId, playerId, { event: 'PlayDefuse', data: { card_idx: defuseIdx, insert_depth: randomDepth } });
      showToast(`Defused! Kitten hidden at depth ${randomDepth}.`);
    } catch (e: any) { showToast(e.message, 'error'); }
    finally { setIsActionPending(false); }
  };

  const handleAcceptFate = async () => {
    if (isActionPending) return;
    setIsActionPending(true);
    try {
      await api.move(gameId, playerId, { event: 'TimerExpired' });
    } catch (e: any) { showToast(e.message, 'error'); }
    finally { setIsActionPending(false); }
  };

  return (
    <div className="game-layout">
      <div className="toast-area">
        {toasts.map(t => <div key={t.id} className={`toast ${t.type}`}>{t.msg}</div>)}
      </div>

      <div className="header">
        <div style={{ display: 'flex', gap: 15, alignItems: 'center' }}>
          <div style={{ fontWeight: 800, fontSize: '1.2rem', color: '#ef4444' }}>💣 KITTENS</div>
          <button className="log-toggle-btn" onClick={() => setShowLogs(!showLogs)}>
            {showLogs ? 'Hide Logs' : '📜 Logs'}
          </button>
        </div>

        <div className={`turn-badge ${isMyTurn ? 'my-turn' : ''}`}>
          {isMyTurn ? "YOUR TURN" : `${state.players[state.current_player_idx]?.name}'s Turn`}
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 15 }}>
          <div style={{ fontSize: '0.9rem', opacity: 0.7 }}>Room: {gameId.slice(0, 4)}</div>
          <div onClick={handleExit} style={{ cursor: 'pointer', color: '#94a3b8', display: 'flex', alignItems: 'center' }} title="Exit">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path><polyline points="16 17 21 12 16 7"></polyline><line x1="21" y1="12" x2="9" y2="12"></line></svg>
          </div>
        </div>
      </div>

      {showLogs && (
        <div className="logs-modal">
          <div className="logs-header">
            <span>Audit Logs</span>
            <span style={{ cursor: 'pointer' }} onClick={() => setShowLogs(false)}>✕</span>
          </div>
          <div className="logs-list">
            {state.logs.map((l, i) => (
              <div key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.1)', paddingBottom: 4 }}>
                <span style={{ color: '#64748b', marginRight: 5 }}>[{new Date(l.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}]</span>
                {l.message}
              </div>
            ))}
            <div ref={logsEndRef} />
          </div>
        </div>
      )}

      <div className="table-area">
        <div className="opponents-row">
          {state.players.map((p, i) => {
            if (i === myIdx) return null;
            const isTargetable = targetMode !== null;
            return (
              <div
                key={i}
                className={`opponent-card ${i === state.current_player_idx ? 'active-turn' : ''} ${isTargetable ? 'selectable' : ''}`}
                onClick={() => isTargetable && handleTargetClick(i)}
              >
                <span className="avatar">{p.is_eliminated ? '💀' : '👤'}</span>
                <div style={{ fontWeight: 'bold', fontSize: '0.9rem' }}>{p.name}</div>
                <div style={{ fontSize: '0.8rem', opacity: 0.7 }}>{p.hand_count} Cards</div>
              </div>
            )
          })}
        </div>

        <div className="deck-area">
          <div className="card-stack deck" onClick={isMyTurn && !isExploding ? handleDraw : undefined}>
            <div style={{ color: 'white', fontWeight: 'bold', fontSize: '1.2rem' }}>DECK</div>
            <div>{state.deck_count}</div>
            {isMyTurn && !isExploding && !targetMode && <div style={{ fontSize: '0.7rem', marginTop: 5 }}>DRAW</div>}
          </div>
          <div className="card-stack discard">
            <div style={{ fontWeight: 'bold', fontSize: '0.9rem' }}>DISCARD</div>
            {state.discard_pile.length > 0 ? (
              <div style={{ marginTop: 10, fontSize: '1.2rem' }}>
                {state.discard_pile[state.discard_pile.length - 1].kind.type}
              </div>
            ) : <div style={{ opacity: 0.5 }}>Empty</div>}
          </div>
        </div>

        {state.last_action_result && (
          <div className="toast" style={{ marginTop: 20, background: '#8b5cf6' }}>
            🔮 Future: {state.last_action_result}
          </div>
        )}
      </div>

      <div className="player-controls">
        <div className="action-bar">
          {isExploding && isMyTurn ? (
            <>
              {hasDefuse ? (
                <button className="btn-action btn-danger" onClick={handleDefuse} disabled={isActionPending}>
                  {isActionPending ? 'Processing...' : 'USE DEFUSE CARD'}
                </button>
              ) : (
                <button className="btn-action btn-danger" onClick={handleAcceptFate} disabled={isActionPending}>
                  {isActionPending ? '...' : 'ACCEPT FATE ☠️'}
                </button>
              )}
            </>
          ) : (
            <>
              {targetMode && <div className="turn-badge my-turn">Select a player to target...</div>}
              {!targetMode && isMyTurn && !isEliminated && (
                <button
                  className="btn-action"
                  disabled={actionType === 'None' || isActionPending}
                  onClick={executeAction}
                >
                  {isActionPending ? '...' : actionLabel}
                </button>
              )}
              {targetMode && <button className="btn-action" onClick={() => setTargetMode(null)}>Cancel</button>}
            </>
          )}
        </div>

        <div className="hand-container">
          {myHand.map((card, idx) => {
            const isSelected = selected.includes(idx);
            const total = myHand.length;
            const center = (total - 1) / 2;
            const rotate = (idx - center) * 5;
            const translateY = isSelected ? -50 : Math.abs(idx - center) * 5;

            return (
              <div
                key={card.id}
                className={`card ${card.kind.type} ${isSelected ? 'selected' : ''}`}
                style={{ transform: `rotate(${rotate}deg) translateY(${translateY}px)` }}
                onClick={() => {
                  if (selected.includes(idx)) setSelected(s => s.filter(i => i !== idx));
                  else setSelected(s => [...s, idx]);
                }}
              >
                <div style={{ fontWeight: 700, fontSize: '0.8rem', textAlign: 'left' }}>{card.kind.type}</div>
                <div style={{ textAlign: 'center', fontSize: '2rem', opacity: 0.8 }}>
                  {['Attack', 'Skip', 'Favor'].includes(card.kind.type) ? '⚡️' :
                    card.kind.type === 'Defuse' ? '🔧' :
                      card.kind.type === 'ExplodingKitten' ? '💣' : '🐱'}
                </div>
                <div style={{ fontSize: '0.7rem', textAlign: 'right', opacity: 0.6 }}>{card.kind.data || ''}</div>
              </div>
            );
          })}
        </div>
      </div>

      {/* --- LOBBY / WAITING SCREEN WITH COPY BUTTON --- */}
      {phaseStr === 'WaitingForPlayers' && (
        <div className="overlay">
          <div style={{ background: '#1e293b', padding: 40, borderRadius: 20, textAlign: 'center', minWidth: '350px' }}>
            <h2>Waiting for Players ({state.players.length}/5)</h2>

            {/* GAME ID CARD */}
            <div style={{ background: '#0f172a', padding: '15px', borderRadius: '8px', border: '1px solid #334155', margin: '20px 0' }}>
              <div style={{ fontSize: '0.75rem', textTransform: 'uppercase', color: '#64748b', marginBottom: 5 }}>Invite Friends</div>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', background: '#1e293b', padding: '8px 12px', borderRadius: 4, border: '1px solid #334155' }}>
                <code style={{ color: '#3b82f6', fontWeight: 'bold', fontSize: '0.9rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: '200px' }}>
                  {gameId}
                </code>
                <button
                  onClick={copyGameId}
                  style={{ background: '#3b82f6', color: 'white', fontSize: '0.75rem', padding: '4px 8px', marginLeft: 10 }}
                >
                  COPY
                </button>
              </div>
            </div>

            <div style={{ display: 'flex', gap: 10, justifyContent: 'center', margin: '20px 0', flexWrap: 'wrap' }}>
              {state.players.map(p => <div key={p.id} className="turn-badge">{p.name}</div>)}
            </div>

            {state.players.length >= 2 ? (
              <button className="btn-action" onClick={() => api.start(gameId)}>START GAME</button>
            ) : <p style={{ color: '#94a3b8', fontSize: '0.9rem' }}>Need at least 2 players to start...</p>}
          </div>
        </div>
      )}

      {phaseStr === 'GameOver' && (
        <div className="overlay">
          <h1>🏆 GAME OVER 🏆</h1>
          <p>Check the logs to see who won!</p>
          <button className="btn-action" onClick={() => route('/')}>Back to Home</button>
        </div>
      )}

      {isEliminated && phaseStr !== 'GameOver' && (
        <div className="overlay" style={{ background: 'rgba(0,0,0,0.5)', pointerEvents: 'none' }}>
          <h1 style={{ color: 'red', transform: 'rotate(-10deg)', fontSize: '4rem', textShadow: '0 0 20px black' }}>ELIMINATED</h1>
        </div>
      )}
    </div>
  );
}