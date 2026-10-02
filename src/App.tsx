import { useState } from "react";

export default function App() {
  const [query, setQuery] = useState("");

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand">
          <div className="brand-mark">PD</div>
          <div>
            <strong>Professional DocX</strong>
            <span>Development foundation</span>
          </div>
        </div>

        <nav className="nav-list">
          <button className="nav-item nav-item-active" type="button">
            Home
          </button>
          <button className="nav-item" type="button" disabled>
            Clients
          </button>
          <button className="nav-item" type="button" disabled>
            Engagements
          </button>
          <button className="nav-item" type="button" disabled>
            Recent
          </button>
          <button className="nav-item" type="button" disabled>
            Pinned
          </button>
        </nav>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <label className="search-label" htmlFor="universal-search">
            Universal search
          </label>
          <div className="search-wrap">
            <span aria-hidden="true">⌕</span>
            <input
              id="universal-search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search documents, clients, engagements..."
              autoComplete="off"
            />
            <kbd>Ctrl K</kbd>
          </div>
        </header>

        <section className="hero">
          <p className="eyebrow">FOUNDATION BUILD</p>
          <h1>Reach the right document in seconds.</h1>
          <p className="hero-copy">
            Professional DocX will index existing files in place, make them
            quickly searchable, and preserve controlled evidence only when
            required.
          </p>

          {query ? (
            <div className="status-card" role="status">
              Search UI is ready. Indexing and ranked results are implemented
              in the next development step for <strong>{query}</strong>.
            </div>
          ) : (
            <div className="status-card">
              Phase 1 foundation is active. No client files are being copied or
              indexed yet.
            </div>
          )}
        </section>

        <section className="quick-grid" aria-label="Foundation principles">
          <article>
            <span>01</span>
            <h2>Search first</h2>
            <p>Partial names such as “salamudd” must find the right file.</p>
          </article>
          <article>
            <span>02</span>
            <h2>Files stay in place</h2>
            <p>Existing folders are referenced and indexed, not duplicated.</p>
          </article>
          <article>
            <span>03</span>
            <h2>Evidence when needed</h2>
            <p>Immutable copies are created only for controlled evidence.</p>
          </article>
        </section>
      </main>
    </div>
  );
}
