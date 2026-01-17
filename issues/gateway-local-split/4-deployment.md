# Deployment & Documentation

**Status:** Planned

## Objective

Create deployment artifacts and documentation for the gateway binary, enabling easy deployment on edge/CDN infrastructure.

## Implementation Steps

1. Create Dockerfile for gateway
2. Document gateway configuration options
3. Write deployment guide for edge/CDN setup
4. Update CLI documentation for both binaries
5. Add examples for common deployment scenarios

## Files to Modify/Create

### New Files

| File | Description |
|------|-------------|
| `crates/gateway/Dockerfile` | Container image |
| `docs/gateway-deployment.md` | Deployment guide |

### Modified Files

| File | Changes |
|------|---------|
| `docs/usage.md` | Update for split binaries |
| `README.md` | Update project overview |

## Acceptance Criteria

- [ ] Docker image builds successfully
- [ ] Documentation covers all gateway options
- [ ] Deployment guide includes:
  - [ ] Single-node setup
  - [ ] Multi-node/load-balanced setup
  - [ ] CDN/edge integration
- [ ] Examples are runnable
- [ ] README reflects new architecture

## Verification

```bash
# Build Docker image
docker build -f crates/gateway/Dockerfile -t jax-gateway .

# Run containerized gateway
docker run -p 8080:8080 jax-gateway

# Verify documentation renders
# (manual review)
```
