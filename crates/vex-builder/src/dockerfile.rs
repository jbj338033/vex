use crate::detect::{NodeFramework, NodePackageManager, ProjectType, PythonPackageManager};

pub fn generate(project_type: &ProjectType) -> Option<String> {
    match project_type {
        ProjectType::Dockerfile => None,
        ProjectType::Node {
            package_manager,
            framework,
        } => Some(generate_node(package_manager, framework)),
        ProjectType::Python { package_manager } => Some(generate_python(package_manager)),
        ProjectType::Go => Some(generate_go()),
        ProjectType::Rust => Some(generate_rust()),
        ProjectType::Static => Some(generate_static()),
    }
}

fn generate_node(pm: &NodePackageManager, framework: &NodeFramework) -> String {
    let (install_cmd, copy_lock, run_cmd) = match pm {
        NodePackageManager::Pnpm => (
            "corepack enable && pnpm install --frozen-lockfile",
            "COPY pnpm-lock.yaml ./",
            "pnpm",
        ),
        NodePackageManager::Yarn => (
            "corepack enable && yarn install --frozen-lockfile",
            "COPY yarn.lock ./",
            "yarn",
        ),
        NodePackageManager::Bun => (
            "npm i -g bun && bun install --frozen-lockfile",
            "COPY bun.lockb bun.lock* ./",
            "bunx",
        ),
        NodePackageManager::Npm => ("npm ci", "COPY package-lock.json* ./", "npx"),
    };

    let (build_stage, start_cmd) = match framework {
        NodeFramework::Next => (
            format!("RUN {run_cmd} next build"),
            format!(
                "CMD [\"{}\", \"next\", \"start\", \"-p\", \"${{PORT:-3000}}\"]",
                run_cmd
            ),
        ),
        NodeFramework::Vite => (
            format!("RUN {run_cmd} vite build"),
            "CMD [\"npx\", \"serve\", \"-s\", \"dist\", \"-l\", \"${PORT:-3000}\"]".to_string(),
        ),
        NodeFramework::Remix => (
            format!("RUN {run_cmd} remix build"),
            format!("CMD [\"{}\", \"remix-serve\", \"build/index.js\"]", run_cmd),
        ),
        NodeFramework::Plain => (
            "RUN npm run build --if-present".to_string(),
            "CMD [\"node\", \"index.js\"]".to_string(),
        ),
    };

    format!(
        r#"FROM node:22-slim AS builder
WORKDIR /app
COPY package.json ./
{copy_lock}
RUN {install_cmd}
COPY . .
{build_stage}

FROM node:22-slim
WORKDIR /app
COPY --from=builder /app .
ENV PORT=3000
EXPOSE 3000
{start_cmd}
"#
    )
}

fn generate_python(pm: &PythonPackageManager) -> String {
    match pm {
        PythonPackageManager::Uv => r#"FROM python:3.13-slim
WORKDIR /app
COPY --from=ghcr.io/astral-sh/uv:latest /uv /bin/uv
COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-dev
COPY . .
ENV PORT=8000
EXPOSE 8000
CMD ["uv", "run", "python", "-m", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "${PORT:-8000}"]
"#
        .to_string(),
        PythonPackageManager::Poetry => r#"FROM python:3.13-slim
WORKDIR /app
RUN pip install poetry && poetry config virtualenvs.create false
COPY pyproject.toml poetry.lock ./
RUN poetry install --no-dev --no-interaction
COPY . .
ENV PORT=8000
EXPOSE 8000
CMD ["python", "-m", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "${PORT:-8000}"]
"#
        .to_string(),
        PythonPackageManager::Pip => r#"FROM python:3.13-slim
WORKDIR /app
COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt
COPY . .
ENV PORT=8000
EXPOSE 8000
CMD ["python", "-m", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "${PORT:-8000}"]
"#
        .to_string(),
    }
}

fn generate_go() -> String {
    r#"FROM golang:1.23 AS builder
WORKDIR /app
COPY go.mod go.sum* ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /app/server .

FROM gcr.io/distroless/static-debian12
COPY --from=builder /app/server /server
ENV PORT=8080
EXPOSE 8080
CMD ["/server"]
"#
    .to_string()
}

fn generate_rust() -> String {
    r#"FROM rust:1.83-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
COPY . .
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/* /usr/local/bin/app
ENV PORT=8080
EXPOSE 8080
CMD ["/usr/local/bin/app"]
"#
    .to_string()
}

fn generate_static() -> String {
    r#"FROM nginx:alpine
COPY . /usr/share/nginx/html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dockerfile_project_returns_none() {
        assert!(generate(&ProjectType::Dockerfile).is_none());
    }

    #[test]
    fn node_pnpm_next_generates_dockerfile() {
        let result = generate(&ProjectType::Node {
            package_manager: NodePackageManager::Pnpm,
            framework: NodeFramework::Next,
        });
        let df = result.unwrap();
        assert!(df.contains("pnpm install --frozen-lockfile"));
        assert!(df.contains("next build"));
        assert!(df.contains("\"next\", \"start\""));
    }

    #[test]
    fn python_uv_generates_dockerfile() {
        let result = generate(&ProjectType::Python {
            package_manager: PythonPackageManager::Uv,
        });
        let df = result.unwrap();
        assert!(df.contains("uv sync"));
        assert!(df.contains("astral-sh/uv"));
    }

    #[test]
    fn go_generates_dockerfile() {
        let df = generate(&ProjectType::Go).unwrap();
        assert!(df.contains("go build"));
        assert!(df.contains("distroless"));
    }

    #[test]
    fn rust_generates_dockerfile() {
        let df = generate(&ProjectType::Rust).unwrap();
        assert!(df.contains("cargo build --release"));
    }

    #[test]
    fn static_generates_nginx() {
        let df = generate(&ProjectType::Static).unwrap();
        assert!(df.contains("nginx:alpine"));
    }
}
