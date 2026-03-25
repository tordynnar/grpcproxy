.PHONY: proto test run-backend run-proxy run-client install

install:
	pip install -r requirements.txt

proto:
	python -m grpc_tools.protoc -I../proto \
		--python_out=grpcproxy/generated \
		--grpc_python_out=grpcproxy/generated \
		../proto/echo.proto ../proto/math.proto
	python -c "\
import re, pathlib; \
[pathlib.Path(f).write_text(re.sub(r'^(import (echo|math)_pb2)', r'from grpcproxy.generated \1', pathlib.Path(f).read_text(), flags=re.MULTILINE)) \
 for f in pathlib.Path('grpcproxy/generated').glob('*_grpc.py')]"

test: proto
	python -m pytest tests/ -v

run-backend: proto
	python cmd/run_backend.py

run-proxy: proto
	python cmd/run_proxy.py

run-client: proto
	python cmd/run_client.py
