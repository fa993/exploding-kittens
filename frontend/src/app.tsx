import { Router } from 'preact-router';
import { Home } from './pages/Home';
import { GameBoard } from './pages/GameBoard';
import { base } from './utils';

export function App() {

  return (
    <Router>
      <Home path={`${base}/`} />
      <GameBoard path={`${base}/game/:gameId`} />
      <div default> 404: Unknown Path {window.location.pathname}</div>
    </Router>
  );
}