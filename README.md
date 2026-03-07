## Startup commands

- cd frontend
- npm i
- cd frontend && npm run build && cd .. && RUST_LOG=trace cargo run

## To change base URL

- (export VITE_APP_BASE_PATH=/kittens/; cd frontend && npm run build && cd ../ && cargo run)
- (export VITE_APP_BASE_PATH=/; cd frontend && npm run build && cd ../ && cargo run)
