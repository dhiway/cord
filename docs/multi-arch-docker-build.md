## Multi-Architecture Docker Build for CORD

This guide provides instructions on how to build a multi-architecture Docker image for CORD using Docker Buildx. The build process includes conditional features based on whether the build is happening on a release branch.

### Prerequisites

1. **Docker**: Ensure Docker is installed on your machine. If not, follow the [Docker installation guide for Linux](https://docs.docker.com/engine/install/ubuntu/).

2. **Docker Buildx**: Docker Buildx should be set up and configured. You can enable Buildx by following these instructions:

   ```sh
   # Create a new builder instance
   docker buildx create --name mybuilder --use

   # Inspect the current builder instance
   docker buildx inspect mybuilder --bootstrap
   ```

Docker Hub Login: Ensure you have a Docker Hub account. You need to login to Docker Hub from your CLI before pushing images:

```sh
  docker login -u <username>
```

You will be prompted to enter your password or a Docker Hub access token. It is recommended to use an access token for better security. You can create an access token in your Docker Hub account settings under Security.

### Building the Docker Image

To build the Docker image, use the `docker buildx build` command. The command can be customized based on whether you are building for a release branch or not.

#### For Regular Build

```sh
docker buildx build --platform linux/amd64,linux/arm64 -t dhiway/cord:latest --build-arg CARGO_PROFILE=release --build-arg RELEASE_BRANCH=false --file Dockerfile --push .
```

#### For Release Build (with `--features on-chain-release-build`)

```sh
docker buildx build --platform linux/amd64,linux/arm64 -t dhiway/cord:latest --build-arg CARGO_PROFILE=release --build-arg RELEASE_BRANCH=true --file Dockerfile --push .
```

#### Command Options

- `--platform linux/amd64,linux/arm64`: Specifies the target platforms for a multi-architecture build.
- `-t dhiway/cord:latest`: Tags the built image.
- `--build-arg CARGO_PROFILE=release`: Passes the `CARGO_PROFILE` argument to the Docker build.
- `--build-arg RELEASE_BRANCH=true|false`: Conditionally sets the `RELEASE_BRANCH` argument.
- `--file Dockerfile`: Specifies the Dockerfile to use.
- `--push`: Pushes the built image to the Docker registry.

### Verifying the Build

To verify that the built image works correctly, you can run the Docker container locally:

```sh
docker run -p 30333:30333 -p 9933:9933 -p 9944:9944 dhiway/cord:latest --dev --unsafe-rpc-external --rpc-cors all --name "LoomNode"
```

This command starts the container and exposes the necessary ports, allowing you to test the Cord node.
