/* ============================================================
   ChronoSeal Website — app.js
   Core runtime: header scroll, mobile nav, scroll animations,
   code copy, back-to-top, keyboard shortcuts
   ============================================================ */

(function () {
  'use strict';

  /* ---- Header scroll state ---- */
  var header = document.querySelector('.navbar');
  if (header) {
    var onScroll = function () {
      header.classList.toggle('scrolled', window.scrollY > 20);
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    onScroll();
  }

  /* ---- Enhanced Mobile nav toggle ---- */
  var toggle = document.querySelector('.nav-toggle');
  var navLinks = document.querySelector('.nav-links');
  if (toggle && navLinks) {
    var menuIcon = toggle.querySelector('i');
    toggle.addEventListener('click', function () {
      var isActive = navLinks.classList.contains('active');
      if (isActive) {
        navLinks.classList.remove('active');
        if (menuIcon) {
          menuIcon.classList.remove('fa-times');
          menuIcon.classList.add('fa-bars');
        }
      } else {
        navLinks.classList.add('active');
        if (menuIcon) {
          menuIcon.classList.remove('fa-bars');
          menuIcon.classList.add('fa-times');
        }
      }
    });

    // Close nav on link click
    navLinks.querySelectorAll('a').forEach(function (a) {
      a.addEventListener('click', function () {
        navLinks.classList.remove('active');
        if (menuIcon) {
          menuIcon.classList.remove('fa-times');
          menuIcon.classList.add('fa-bars');
        }
      });
    });
  }

  /* ---- Doc sidebar mobile toggle ---- */
  var sidebarToggle = document.querySelector('.doc-sidebar-toggle');
  var sidebar = document.querySelector('.doc-sidebar');
  if (sidebarToggle && sidebar) {
    sidebarToggle.addEventListener('click', function () {
      sidebar.classList.toggle('open');
    });
    // Close sidebar on link click (mobile)
    sidebar.querySelectorAll('.doc-nav-item').forEach(function (a) {
      a.addEventListener('click', function () {
        if (window.innerWidth <= 900) sidebar.classList.remove('open');
      });
    });
  }

  /* ---- Intersection Observer for Scroll Animations ---- */
  if ('IntersectionObserver' in window) {
    var observerOptions = { threshold: 0.1, rootMargin: '0px 0px -50px 0px' };
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add('in-view');
          observer.unobserve(entry.target);
        }
      });
    }, observerOptions);

    // Observe all animated elements
    document.querySelectorAll('.animate, .card, .stat-card, .step').forEach(function (el) {
      if (!el.classList.contains('animate')) {
        el.classList.add('animate');
      }
      observer.observe(el);
    });
  }

  /* ---- Code block copy buttons ---- */
  document.querySelectorAll('.code-copy-btn').forEach(function (btn) {
    btn.addEventListener('click', function () {
      var pre = btn.closest('.code-block').querySelector('pre');
      if (!pre) return;
      var text = pre.textContent;
      if (navigator.clipboard) {
        navigator.clipboard.writeText(text).then(function () {
          btn.textContent = 'Copied!';
          setTimeout(function () { btn.textContent = 'Copy'; }, 1500);
        });
      } else {
        // Fallback
        var ta = document.createElement('textarea');
        ta.value = text;
        ta.style.cssText = 'position:fixed;left:-999px';
        document.body.appendChild(ta);
        ta.select();
        document.execCommand('copy');
        document.body.removeChild(ta);
        btn.textContent = 'Copied!';
        setTimeout(function () { btn.textContent = 'Copy'; }, 1500);
      }
    });
  });

  /* ---- Back to top ---- */
  var btt = document.querySelector('.back-to-top');
  if (btt) {
    window.addEventListener('scroll', function () {
      btt.classList.toggle('visible', window.scrollY > 400);
    }, { passive: true });
    btt.addEventListener('click', function (e) {
      e.preventDefault();
      window.scrollTo({ top: 0, behavior: 'smooth' });
    });
  }

  /* ---- Keyboard shortcut: / or Ctrl+K for search ---- */
  document.addEventListener('keydown', function (e) {
    if (e.key === '/' && !isInput(e.target)) {
      e.preventDefault();
      if (typeof ChronoSearch !== 'undefined') ChronoSearch.open();
    }
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
      e.preventDefault();
      if (typeof ChronoSearch !== 'undefined') ChronoSearch.open();
    }
    if (e.key === 'Escape') {
      if (typeof ChronoSearch !== 'undefined') ChronoSearch.close();
    }
  });

  function isInput(el) {
    var tag = el.tagName;
    return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable;
  }

  /* ---- Nav dropdown (desktop hover + mobile tap) ---- */
  document.querySelectorAll('.nav-dropdown').forEach(function (dd) {
    var trigger = dd.querySelector('.nav-link');
    if (!trigger) return;
    trigger.addEventListener('click', function (e) {
      if (window.innerWidth <= 768) {
        e.preventDefault();
        dd.classList.toggle('open');
      }
    });
  });

  /* ---- Active nav link highlight ---- */
  var currentPage = window.location.pathname.split('/').pop() || 'index.html';
  document.querySelectorAll('.nav-links a, .doc-nav-item').forEach(function (link) {
    var href = link.getAttribute('href');
    if (!href) return;
    var linkPage = href.split('/').pop().split('#')[0] || 'index.html';
    if (linkPage === currentPage) {
      link.classList.add('active');
    }
  });

  /* ---- Smooth anchor scroll ---- */
  document.querySelectorAll('a[href^="#"]').forEach(function (a) {
    a.addEventListener('click', function (e) {
      var id = a.getAttribute('href').substring(1);
      var target = document.getElementById(id);
      if (target) {
        e.preventDefault();
        target.scrollIntoView({ behavior: 'smooth', block: 'start' });
        history.pushState(null, '', '#' + id);
      }
    });
  });

})();
