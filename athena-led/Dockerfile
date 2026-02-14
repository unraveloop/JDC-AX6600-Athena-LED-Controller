FROM rust:slim

# 安装交叉编译工具和必要的构建工具
RUN apt-get update && apt-get install -y \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    musl-tools \
    pkg-config \
    --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

# 添加目标架构
RUN rustup target add aarch64-unknown-linux-musl

# 创建新的用户和工作目录
RUN useradd -m -u 1000 rust
WORKDIR /home/rust/athena-led

# 设置交叉编译环境变量
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_musl=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_musl=aarch64-linux-gnu-g++

# 创建并设置缓存目录权限
RUN mkdir -p target && chown rust:rust target

# 创建缓存层：复制依赖文件并预编译依赖
COPY --chown=rust:rust Cargo.toml Cargo.lock ./
RUN chown rust:rust . 
USER rust
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --target aarch64-unknown-linux-musl --release && \
    rm -rf src target/aarch64-unknown-linux-musl/release/deps/athena_led*

# 创建构建层：复制源代码并构建
COPY --chown=rust:rust src ./src/
RUN cargo build --target aarch64-unknown-linux-musl --release && \
    mkdir -p /home/rust/release && \
    cp target/aarch64-unknown-linux-musl/release/athena-led /home/rust/release/

# 创建最终镜像
FROM scratch
COPY --from=0 /home/rust/release/athena-led /athena-led
