FROM rust:1.85-alpine AS builder
RUN apk add --no-cache musl-dev pkgconfig openssl-dev
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
WORKDIR /app
COPY --from=builder /app/target/release/cargo-night-server .
COPY .env .
COPY migrations/ migrations/
EXPOSE 8080
CMD ["./cargo-night-server"]
