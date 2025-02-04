FROM debian:bookworm-slim

RUN mkdir -p /solana
WORKDIR /solana

ARG TARGETARCH

COPY output/linux/${TARGETARCH}/block-meta-rpc /solana/block-meta-rpc

EXPOSE 8899

ENV RUST_LOG=info

CMD ["./block-meta-rpc", "--bind-address=0.0.0.0", "--enable-rpc-mysql-meta-storage", "--rpc-mysql-address=mysql:3306"]