# Instagram post automation app

Creator-facing web application skeleton for planning, reviewing, and publishing Instagram posts.

## Stack

- Frontend: React, Vite, TypeScript, Tailwind CSS
- Backend: Rust, Axum, Tokio
- Persistent state: PostgreSQL in later data-model work

## Development

Copy the environment template and fill in local values:

```bash
cp .env.example .env
```

Install frontend dependencies:

```bash
npm install
```

Run the backend on `0.0.0.0:8080`:

```bash
export DATABASE_URL=...
cargo run -p instagram-auto-backend
```

Run the frontend dev server:

```bash
npm run dev --workspace frontend
```

Build everything:

```bash
npm run build --workspace frontend
cargo build
```

Apply PostgreSQL migrations:

```bash
export DATABASE_URL=...
cargo run -p instagram-auto-backend --bin migrate
```
