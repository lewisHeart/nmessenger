CARGO := env -u RUSTUP_FORCE_ARG0 cargo
export NM_TEST_POSTGRES_PORT ?= 55439
export NM_TEST_REDIS_PORT ?= 56379
TEST_DB := postgres://nmessenger:nmessenger_test@127.0.0.1:$(NM_TEST_POSTGRES_PORT)/nmessenger_test
TEST_REDIS := redis://127.0.0.1:$(NM_TEST_REDIS_PORT)/0
ANDROID_ARGS ?=

.PHONY: up down infra server client build build-client build-server check test test-infra test-stop smoke media-test android android-debug android-release android-check

android: android-debug

android-debug:
	python3 scripts/build_android.py --debug $(ANDROID_ARGS)

android-release:
	python3 scripts/build_android.py --production $(ANDROID_ARGS)

android-check:
	python3 scripts/build_android.py --check $(ANDROID_ARGS)

build: build-server build-client

build-server:
	$(CARGO) build --locked --release -p nm-servo

build-client:
	$(CARGO) build --locked --release -p nmessenger-client

up:
	docker compose up -d --build --wait backend

down:
	docker compose down

infra:
	docker compose up -d --wait postgres redis

server:
	DATABASE_URL=$${DATABASE_URL:-postgres://nmessenger:nmessenger_dev@127.0.0.1:54329/nmessenger} REDIS_URL=$${REDIS_URL:-redis://127.0.0.1:63799/0} BIND_ADDR=$${BIND_ADDR:-127.0.0.1:18087} $(CARGO) run -p nm-servo

client:
	$(CARGO) run -p nmessenger-client --features native-audio

check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings
	python3 scripts/check_limits.py

test-infra:
	docker compose --profile test up -d --wait test-postgres test-redis

test: test-infra
	NM_TEST_DATABASE_URL=$(TEST_DB) NM_TEST_REDIS_URL=$(TEST_REDIS) $(CARGO) test --workspace --all-features -- --test-threads=1

media-test: test-infra
	NM_TEST_DATABASE_URL=$(TEST_DB) NM_TEST_REDIS_URL=$(TEST_REDIS) NM_TEST_MEDIA=1 $(CARGO) test -p nmessenger-client --all-features synthetic_audio_video_cross_the_real_backend_and_survive_restart -- --ignored --test-threads=1

smoke: test-infra
	$(CARGO) build --workspace --all-features
	NM_TEST_DATABASE_URL=$(TEST_DB) NM_TEST_REDIS_URL=$(TEST_REDIS) NM_SMOKE_XVFB=1 xvfb-run -a -s '-screen 0 1280x900x24 -noreset' python3 scripts/smoke_ui.py
	NM_SMOKE_XVFB=1 xvfb-run -a -s '-screen 0 1280x900x24 -noreset' $(CARGO) test -p nmessenger-client --all-features x11_capture_and_revocable_remote_input -- --ignored

test-stop:
	docker compose --profile test stop test-postgres test-redis
