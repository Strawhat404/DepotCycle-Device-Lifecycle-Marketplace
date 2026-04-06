FROM rust:1.87 AS builder
WORKDIR /app
COPY Cargo.toml /app/Cargo.toml
COPY Cargo.lock /app/Cargo.lock
COPY backend /app/backend
COPY frontend /app/frontend
COPY unit_tests /app/unit_tests
COPY API_tests /app/API_tests
RUN cargo build -p backend --release

FROM rust:1.87
WORKDIR /app
ENV DATABASE_URL=sqlite:///data/depotcycle.db
ENV UPLOAD_DIR=/app/uploads
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /data /app/uploads
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
EXPOSE 3000
CMD ["backend"]
