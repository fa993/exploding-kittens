import { Router } from 'preact-router';
import { Home } from './pages/Home';
import { GameBoard } from './pages/GameBoard';

export function App() {
  return (
    <Router>
      <Home path="/" />
      <GameBoard path="/game/:gameId" />
    </Router>
  );
}