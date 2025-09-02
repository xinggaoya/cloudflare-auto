## 多阶段构建：构建 Rust 可执行文件并打包为精简运行镜像

# ========== 构建阶段 ==========
FROM rust:1.79 as builder

WORKDIR /app

# 预复制依赖清单以缓存依赖构建
COPY Cargo.toml Cargo.lock ./

# 创建虚拟源码以触发依赖缓存（构建一次空项目依赖）
RUN mkdir -p src && echo "fn main(){}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# 复制实际源码
COPY src ./src
COPY static ./static

# 构建发布版本
RUN cargo build --release


# ========== 运行阶段 ==========
# 使用 debian slim，包含 glibc 与必要的运行时，体积较小
FROM debian:bookworm-slim AS runtime

ENV APP_USER=appuser \
    APP_HOME=/app \
    RUST_LOG=info \
    BIND_ADDR=0.0.0.0:3000

WORKDIR ${APP_HOME}

# 安装运行所需依赖：CA证书、时区、sqlite3 动态库等
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates tzdata libsqlite3-0 && \
    rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN groupadd -r ${APP_USER} && useradd -r -g ${APP_USER} ${APP_USER}

# 拷贝可执行文件与静态资源
COPY --from=builder /app/target/release/cloudflare-auto /usr/local/bin/cloudflare-auto
COPY --from=builder /app/static ./static

# 数据与日志目录（持久化）
RUN mkdir -p ${APP_HOME}/data ${APP_HOME}/logs && \
    chown -R ${APP_USER}:${APP_USER} ${APP_HOME}

USER ${APP_USER}

EXPOSE 3000

# 运行时将数据库放在 /app/data/config.db，容器外可挂载
VOLUME ["/app/data", "/app/logs"]

ENTRYPOINT ["/usr/local/bin/cloudflare-auto"]

