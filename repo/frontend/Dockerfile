FROM rust:1.88 AS builder
ARG PUBLIC_API_BASE_URL=http://localhost:3000
WORKDIR /app
ENV PUBLIC_API_BASE_URL=${PUBLIC_API_BASE_URL}
RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk --version 0.21.5 --locked
COPY Cargo.toml /app/Cargo.toml
COPY backend /app/backend
COPY frontend /app/frontend
COPY unit_tests /app/unit_tests
COPY API_tests /app/API_tests
WORKDIR /app/frontend
RUN cargo build -p frontend --target wasm32-unknown-unknown --release
RUN CARGO_LOG=cargo::core::compiler::fingerprint=info trunk build --release

FROM nginx:1.27-alpine
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/frontend/dist /usr/share/nginx/html
EXPOSE 8080
