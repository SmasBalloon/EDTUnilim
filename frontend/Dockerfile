# syntax=docker/dockerfile:1

# --- Construction du bundle -------------------------------------------------
FROM node:26-alpine AS builder
WORKDIR /build

COPY package.json package-lock.json ./
RUN npm ci

COPY tsconfig.json vite.config.ts index.html ./
COPY public ./public
COPY src ./src

# Vide par défaut : le front tape `/api/...` sur sa propre origine, que nginx
# relaie vers le service `api`.
ARG VITE_API_URL=""
ENV VITE_API_URL=$VITE_API_URL
RUN npm run build

# --- Service statique -------------------------------------------------------
FROM nginx:alpine AS runtime
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /build/dist /usr/share/nginx/html

EXPOSE 80
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    # 127.0.0.1 et non `localhost` : nginx n'écoute qu'en IPv4, et la résolution
    # de `localhost` commence par ::1 — la sonde serait refusée.
    CMD wget -q --spider http://127.0.0.1/ || exit 1
