#!/bin/bash

set -e  # Exit on error

# 颜色输出
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 架构信息
TARGET_TRIPLE="aarch64-unknown-linux-musl"

# 输出目录
OUTPUT_DIR="output/${TARGET_TRIPLE}"

echo -e "${BLUE}Step 1: Creating output directory...${NC}"
mkdir -p "$OUTPUT_DIR"

echo -e "${BLUE}Step 2: Building Docker image...${NC}"
podman build -t athena-led-builder .

echo -e "${BLUE}Step 3: Extracting binary...${NC}"
podman create --name athena-led-builder-tmp athena-led-builder
podman cp athena-led-builder-tmp:/athena-led "$OUTPUT_DIR/"
podman rm -f athena-led-builder-tmp

# 显示编译结果
echo -e "${GREEN}Build completed successfully!${NC}"
echo -e "${BLUE}Binary location: ${NC}$OUTPUT_DIR/athena-led"
ls -lh "$OUTPUT_DIR/athena-led"
