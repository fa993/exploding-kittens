# ==========================================
# Stage 1: Build Frontend (Node.js)
# ==========================================
FROM node:20-alpine AS frontend-builder

WORKDIR /app

# 1. Copy package files first to leverage Docker cache
COPY frontend/package*.json ./frontend/

# 2. Install dependencies
WORKDIR /app/frontend
RUN npm ci

# 3. Copy the rest of the frontend source
COPY frontend/ .

# 4. Build the frontend
# Note: Your vite.config.ts is set to output to '../dist' (which is /app/dist)
RUN npm run build


# ==========================================
# Stage 2: Build Backend (Rust)
# ==========================================
FROM rust:1.92-alpine AS backend-builder

# Install C compiler essentials for Alpine (musl)
RUN apk add --no-cache musl-dev

WORKDIR /app

# 1. Create a dummy project to cache Cargo dependencies
# This prevents re-downloading crates every time you change a single line of code
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release

# 2. Copy the actual source code
COPY src ./src

# 3. Touch main.rs to force a rebuild of your code (not the deps)
RUN touch src/main.rs
RUN cargo build --release

# 4. Strip debug symbols to reduce image size
# Replace "exploding-kittens" with your actual package name from Cargo.toml if different
RUN strip target/release/exploding-kittens


# ==========================================
# Stage 3: Final Runtime Image
# ==========================================
FROM alpine:3.19

WORKDIR /app

# Install CA certificates (good practice for HTTPS)
RUN apk add --no-cache ca-certificates

# 1. Copy the compiled Rust binary
COPY --from=backend-builder /app/target/release/exploding-kittens ./server

# 2. Copy the built Frontend static files
COPY --from=frontend-builder /app/dist ./dist

# 3. Configuration
ENV RUST_LOG=info
ENV PORT=3000

# 4. Expose the port
EXPOSE 3000

# 5. Start the server
CMD ["./server"]