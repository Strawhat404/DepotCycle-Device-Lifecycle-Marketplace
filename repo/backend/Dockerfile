FROM rust:1.88 AS builder
WORKDIR /app
COPY Cargo.toml /app/Cargo.toml
COPY backend /app/backend
COPY frontend /app/frontend
COPY unit_tests /app/unit_tests
COPY API_tests /app/API_tests
RUN cargo build -p backend --release

FROM rust:1.88
WORKDIR /app
ENV DATABASE_URL=sqlite:///data/depotcycle.db
ENV UPLOAD_DIR=/app/uploads
RUN mkdir -p /data /app/uploads
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
EXPOSE 3000
CMD ["backend"]
