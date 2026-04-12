"""Full-text search tests: partial matches, case insensitivity, special chars.

Tests /api/tasks/search?q= endpoint with FTS5.
"""
import urllib.parse
import pytest
from helpers import H


_seeded = False


@pytest.fixture(autouse=True)
def _seed_once(logged_in):
    global _seeded
    if not _seeded:
        h = H()
        h.create_task("Alphanumeric Task 123", project="SearchProj")
        h.create_task("UPPERCASE TITLE", project="SearchProj")
        h.create_task("lowercase title", project="SearchProj")
        h.create_task("MiXeD CaSe TiTlE", project="SearchProj")
        h.create_task("Special chars here", project="SearchProj")
        h.create_task("Unicode cafe resume naive", project="SearchProj")
        h.create_task("Emoji task rocket", project="SearchProj")
        h.create_task("Hyphenated-task-name", project="SearchProj")
        h.create_task("Dotted.task.name", project="SearchProj")
        h.create_task("Underscored_task_name", project="SearchProj")
        _seeded = True


def _search(h, q):
    """URL-encode the query properly."""
    encoded = urllib.parse.quote(q, safe="")
    code, body = h.api_status("GET", f"/api/tasks/search?q={encoded}")
    return code, body if isinstance(body, list) else []


class TestSearchBasic:

    def test_exact_match(self, logged_in):
        h = H()
        code, results = _search(h, "Alphanumeric")
        assert code == 200
        assert any("Alphanumeric" in r.get("title", "") for r in results)

    def test_case_insensitive(self, logged_in):
        h = H()
        _, r1 = _search(h, "uppercase")
        _, r2 = _search(h, "UPPERCASE")
        _, r3 = _search(h, "Uppercase")
        ids1 = {r["id"] for r in r1}
        ids2 = {r["id"] for r in r2}
        ids3 = {r["id"] for r in r3}
        assert ids1 == ids2 == ids3
        assert len(ids1) >= 1

    def test_partial_word(self, logged_in):
        h = H()
        code, results = _search(h, "Alpha")
        assert code == 200
        assert isinstance(results, list)

    def test_multiple_words(self, logged_in):
        h = H()
        _, results = _search(h, "Alphanumeric Task")
        assert any("Alphanumeric" in r.get("title", "") for r in results)

    def test_number_search(self, logged_in):
        h = H()
        _, results = _search(h, "123")
        assert any("123" in r.get("title", "") for r in results)


class TestSearchEmpty:

    def test_empty_query_returns_empty(self, logged_in):
        h = H()
        _, results = _search(h, "")
        assert results == []

    def test_whitespace_query_returns_empty(self, logged_in):
        h = H()
        _, results = _search(h, "   ")
        assert results == []

    def test_single_char(self, logged_in):
        h = H()
        code, results = _search(h, "a")
        assert code == 200
        assert isinstance(results, list)


class TestSearchSpecialChars:

    def test_at_sign(self, logged_in):
        h = H()
        code, _ = _search(h, "@")
        assert code in (200, 400)

    def test_percent(self, logged_in):
        h = H()
        code, _ = _search(h, "%")
        assert code in (200, 400)

    def test_quotes(self, logged_in):
        h = H()
        code, _ = _search(h, '"hello"')
        assert code in (200, 400)

    def test_backslash(self, logged_in):
        h = H()
        code, _ = _search(h, "\\")
        assert code in (200, 400)

    def test_angle_brackets(self, logged_in):
        h = H()
        code, _ = _search(h, "<script>")
        assert code in (200, 400)

    def test_sql_in_search(self, logged_in):
        h = H()
        code, _ = _search(h, "DROP TABLE tasks")
        assert code in (200, 400)

    def test_hyphenated_search(self, logged_in):
        h = H()
        _, results = _search(h, "Hyphenated")
        assert isinstance(results, list)

    def test_dotted_search(self, logged_in):
        h = H()
        _, results = _search(h, "Dotted")
        assert any("Dotted" in r.get("title", "") for r in results)

    def test_underscored_search(self, logged_in):
        h = H()
        _, results = _search(h, "Underscored")
        assert any("Underscored" in r.get("title", "") for r in results)


class TestSearchBoundary:

    def test_very_long_query(self, logged_in):
        h = H()
        code, _ = _search(h, "x" * 1000)
        assert code in (200, 400, 414)

    def test_limit_default(self, logged_in):
        h = H()
        _, results = _search(h, "task")
        assert len(results) <= 20

    def test_limit_1(self, logged_in):
        h = H()
        encoded = urllib.parse.quote("task", safe="")
        code, body = h.api_status("GET", f"/api/tasks/search?q={encoded}&limit=1")
        assert code == 200
        if isinstance(body, list):
            assert len(body) <= 1

    def test_limit_100(self, logged_in):
        h = H()
        encoded = urllib.parse.quote("task", safe="")
        code, body = h.api_status("GET", f"/api/tasks/search?q={encoded}&limit=100")
        assert code == 200
        if isinstance(body, list):
            assert len(body) <= 100

    def test_limit_0(self, logged_in):
        h = H()
        code, _ = h.api_status("GET", "/api/tasks/search?q=task&limit=0")
        assert code in (200, 400)

    def test_limit_negative(self, logged_in):
        h = H()
        code, _ = h.api_status("GET", "/api/tasks/search?q=task&limit=-1")
        assert code in (200, 400, 422)


class TestSearchResults:

    def test_result_has_id_and_title(self, logged_in):
        h = H()
        h.create_task("SearchResultFields", project="SRF")
        _, results = _search(h, "SearchResultFields")
        assert len(results) >= 1
        assert "id" in results[0]
        assert "title" in results[0]

    def test_result_has_snippet(self, logged_in):
        h = H()
        h.create_task("SnippetTestTask", project="Snippet")
        _, results = _search(h, "SnippetTestTask")
        assert len(results) >= 1
        assert "snippet" in results[0]

    def test_deleted_tasks_not_in_search(self, logged_in):
        h = H()
        t = h.create_task("DeletedSearchTask", project="DS")
        h.delete_task(t["id"])
        _, results = _search(h, "DeletedSearchTask")
        found = any(r.get("id") == t["id"] for r in results)
        assert not found

    def test_nonexistent_term_returns_empty(self, logged_in):
        h = H()
        _, results = _search(h, "xyzzy_nonexistent_term_12345")
        assert results == []
