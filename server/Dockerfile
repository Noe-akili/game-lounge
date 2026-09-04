FROM node:22-alpine

WORKDIR /app

COPY server/package*.json ./
RUN npm ci --omit=dev

COPY server/ ./

ENV PORT=3001
ENV NODE_ENV=production

EXPOSE 3001

CMD ["node", "server.js"]