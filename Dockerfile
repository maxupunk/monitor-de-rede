FROM node:24-alpine AS base

# Stage 1: Build application
FROM base AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN node ace build --tsconfig=tsconfig.build.json

# Stage 2: Production runner
FROM base AS runner
WORKDIR /app
ENV NODE_ENV=production
COPY --from=builder /app/build ./
RUN npm ci --omit="dev"

EXPOSE 3333
CMD ["node", "bin/server.js"]
