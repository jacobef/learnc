FROM node:24-alpine
WORKDIR /app
COPY --chown=node:node package.json server.ts validation.ts storage.ts ./
USER node
ENV NODE_ENV=production
CMD ["node", "server.ts"]
