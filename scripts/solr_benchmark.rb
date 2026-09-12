#!/usr/bin/env ruby
# frozen_string_literal: true

require "csv"
require "json"
require "net/http"
require "uri"
require "yaml"

default_query_file = if File.file?("docs/handoff/SOLR_EVALUATION_QUERIES.yml")
                       "docs/handoff/SOLR_EVALUATION_QUERIES.yml"
                     else
                       "apps/schedule_web/docs/handoff/SOLR_EVALUATION_QUERIES.yml"
                     end
query_file = ARGV.fetch(0, default_query_file)
solr_url = ENV.fetch("SOLR_URL", "http://localhost:8983/solr/schedule_experiment/select")
# The bundled fixture defines and indexes name_txt. Keep the override for
# collections with a different schema, but make the default run meaningful.
query_field = ENV.fetch("SOLR_QUERY_FIELD", "name_txt").strip
iterations = Integer(ENV.fetch("SOLR_ITERATIONS", "20"))
warmup = Integer(ENV.fetch("SOLR_WARMUP", "2"))
raise ArgumentError, "SOLR_ITERATIONS must be positive" unless iterations.positive?

queries = YAML.load_file(query_file).fetch("queries")
uri = URI(solr_url)

def escape_solr(text)
  text.to_s.gsub(/([+\-!(){}\[\]^"~*?:\\\/]|&&|\|\|)/) { |match| "\\#{match}" }
end

def request(uri, text, query_field)
  started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  escaped = escape_solr(text)
  query = query_field.empty? ? escaped : "#{query_field}:(#{escaped})"
  params = URI.encode_www_form(q: query, rows: 5, wt: "json")
  response = Net::HTTP.get_response(URI("#{uri}?#{params}"))
  elapsed_ms = (Process.clock_gettime(Process::CLOCK_MONOTONIC) - started) * 1_000
  raise "Solr returned #{response.code}" unless response.is_a?(Net::HTTPSuccess)

  [elapsed_ms, JSON.parse(response.body)]
end

rows = []
queries.each do |query|
  warmup.times { request(uri, query.fetch("text"), query_field) }
  samples = []
  response = nil
  iterations.times do
    elapsed, response = request(uri, query.fetch("text"), query_field)
    samples << elapsed
  end
  samples.sort!
  docs = response.dig("response", "docs") || []
  rows << {
    "id" => query.fetch("id"),
    "entity" => query.fetch("entity"),
    "iterations" => iterations,
    "p50_latency_ms" => samples[(samples.length * 0.50).floor].round(2),
    "p95_latency_ms" => samples[[(samples.length * 0.95).ceil - 1, 0].max].round(2),
    "result_ids" => docs.filter_map { |doc| doc["id"] || doc["_id"] }
  }
end

puts JSON.pretty_generate({ "solr_url" => solr_url, "query_field" => query_field, "results" => rows })
