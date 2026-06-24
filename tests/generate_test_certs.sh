#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${1:?usage: generate_test_certs.sh OUT_DIR}"
mkdir -p "${OUT_DIR}"

cat > "${OUT_DIR}/server.ext" <<'EOF'
subjectAltName = DNS:localhost,IP:127.0.0.1
extendedKeyUsage = serverAuth
keyUsage = digitalSignature,keyEncipherment
EOF

cat > "${OUT_DIR}/client.ext" <<'EOF'
extendedKeyUsage = clientAuth
keyUsage = digitalSignature,keyEncipherment
EOF

openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout "${OUT_DIR}/ca.key" \
  -out "${OUT_DIR}/ca.pem" \
  -subj "/CN=SDR Software Sim Test CA" \
  -days 1

openssl req -newkey rsa:2048 -nodes \
  -keyout "${OUT_DIR}/server.key" \
  -out "${OUT_DIR}/server.csr" \
  -subj "/CN=localhost"

openssl x509 -req \
  -in "${OUT_DIR}/server.csr" \
  -CA "${OUT_DIR}/ca.pem" \
  -CAkey "${OUT_DIR}/ca.key" \
  -CAcreateserial \
  -out "${OUT_DIR}/server.pem" \
  -days 1 \
  -extfile "${OUT_DIR}/server.ext"

openssl req -newkey rsa:2048 -nodes \
  -keyout "${OUT_DIR}/client.key" \
  -out "${OUT_DIR}/client.csr" \
  -subj "/CN=software-sim"

openssl x509 -req \
  -in "${OUT_DIR}/client.csr" \
  -CA "${OUT_DIR}/ca.pem" \
  -CAkey "${OUT_DIR}/ca.key" \
  -CAcreateserial \
  -out "${OUT_DIR}/client.pem" \
  -days 1 \
  -extfile "${OUT_DIR}/client.ext"

openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout "${OUT_DIR}/wrong-ca.key" \
  -out "${OUT_DIR}/wrong-ca.pem" \
  -subj "/CN=SDR Wrong Test CA" \
  -days 1

openssl req -newkey rsa:2048 -nodes \
  -keyout "${OUT_DIR}/wrong-client.key" \
  -out "${OUT_DIR}/wrong-client.csr" \
  -subj "/CN=wrong-software-sim"

openssl x509 -req \
  -in "${OUT_DIR}/wrong-client.csr" \
  -CA "${OUT_DIR}/wrong-ca.pem" \
  -CAkey "${OUT_DIR}/wrong-ca.key" \
  -CAcreateserial \
  -out "${OUT_DIR}/wrong-client.pem" \
  -days 1 \
  -extfile "${OUT_DIR}/client.ext"
