FROM node:22-alpine
WORKDIR /app
COPY package.json ./
RUN npm install --omit=dev --ignore-scripts
COPY server.js ./
COPY public ./public
# PORT is injected by the host (Railway). Do not bake secrets or pin PORT.
CMD ["node", "server.js"]
