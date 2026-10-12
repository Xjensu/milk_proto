FROM python:3.11.11-slim-bookworm@sha256:081075da77b2b55c23c088251026fb69a7b2bf92471e491ff5fd75c192fd38e5

ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 SCHEDULER_PORT=8100
WORKDIR /app
COPY requirements.txt ./
RUN pip install --no-cache-dir --requirement requirements.txt
COPY analyzer.py preference_compiler.py solver.py server.py ./
EXPOSE 8100
USER 10001:10001
CMD ["python", "server.py"]
