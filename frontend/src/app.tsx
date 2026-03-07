import { Router } from 'preact-router';
import { Home } from './pages/Home';
import { GameBoard } from './pages/GameBoard';

export function App() {
  const base = import.meta.env.BASE_URL.replace(/\/$/, '');

  return (
    <Router>
      <Home path={`${base}/`} />
      <GameBoard path={`${base}/game/:gameId`} />
      <div default> 404: Unknown Path {window.location.pathname}</div>
    </Router>
  );
}