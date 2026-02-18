FROM node:22-alpine AS base

WORKDIR /app

COPY package.json ./
RUN npm install --production=false

COPY . .
RUN npx tsup src/index.ts --format esm

FROM node:22-alpine AS runner

WORKDIR /app

COPY --from=base /app/package.json ./
RUN npm install --production
COPY --from=base /app/dist ./dist
COPY --from=base /app/drizzle ./drizzle
COPY --from=base /app/src/graph/fonts ./src/graph/fonts

CMD ["node", "dist/index.js"]
