/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

/**
 * @file ResvgQt.h
 *
 * Qt API for resvg-qt
 */

#ifndef RESVGQT_H
#define RESVGQT_H

#define RESVGQT_MAJOR_VERSION 0
#define RESVGQT_MINOR_VERSION 9
#define RESVGQT_PATCH_VERSION 1
#define RESVGQT_VERSION "0.9.1"

#include <QDebug>
#include <QFile>
#include <QGuiApplication>
#include <QPainter>
#include <QRectF>
#include <QScopedPointer>
#include <QScreen>
#include <QString>
#include <QTransform>

namespace ResvgPrivate {

extern "C" {
    typedef struct resvg_render_tree resvg_render_tree;

    typedef enum resvg_error {
        RESVG_OK = 0,
        RESVG_ERROR_NOT_AN_UTF8_STR,
        RESVG_ERROR_FILE_WRITE_FAILED,
        RESVG_ERROR_INVALID_FILE_SUFFIX,
        RESVG_ERROR_MALFORMED_GZIP,
        RESVG_ERROR_INVALID_SIZE,
        RESVG_ERROR_PARSING_FAILED,
    } resvg_error;

    typedef struct resvg_color {
        uint8_t r;
        uint8_t g;
        uint8_t b;
    } resvg_color;

    typedef enum resvg_fit_to_type {
        RESVG_FIT_TO_ORIGINAL,
        RESVG_FIT_TO_WIDTH,
        RESVG_FIT_TO_HEIGHT,
        RESVG_FIT_TO_ZOOM,
    } resvg_fit_to_type;

    typedef struct resvg_fit_to {
        resvg_fit_to_type type;
        float value;
    } resvg_fit_to;

    typedef enum resvg_shape_rendering {
        RESVG_SHAPE_RENDERING_OPTIMIZE_SPEED,
        RESVG_SHAPE_RENDERING_CRISP_EDGES,
        RESVG_SHAPE_RENDERING_GEOMETRIC_PRECISION,
    } resvg_shape_rendering;

    typedef enum resvg_text_rendering {
        RESVG_TEXT_RENDERING_OPTIMIZE_SPEED,
        RESVG_TEXT_RENDERING_OPTIMIZE_LEGIBILITY,
        RESVG_TEXT_RENDERING_GEOMETRIC_PRECISION,
    } resvg_text_rendering;

    typedef enum resvg_image_rendering {
        RESVG_IMAGE_RENDERING_OPTIMIZE_QUALITY,
        RESVG_IMAGE_RENDERING_OPTIMIZE_SPEED,
    } resvg_image_rendering;

    typedef struct resvg_options {
        const char *path;
        double dpi;
        const char *font_family;
        double font_size;
        const char *languages;
        resvg_shape_rendering shape_rendering;
        resvg_text_rendering text_rendering;
        resvg_image_rendering image_rendering;
        resvg_fit_to fit_to;
        bool draw_background;
        resvg_color background;
        bool keep_named_groups;
    } resvg_options;

    typedef struct resvg_rect {
        double x;
        double y;
        double width;
        double height;
    } resvg_rect;

    typedef struct resvg_size {
        uint32_t width;
        uint32_t height;
    } resvg_size;

    typedef struct resvg_transform {
        double a;
        double b;
        double c;
        double d;
        double e;
        double f;
    } resvg_transform;

    void resvg_init_log();
    void resvg_init_options(resvg_options *opt);
    int resvg_parse_tree_from_file(const char *file_path,
                                   const resvg_options *opt,
                                   resvg_render_tree **tree);
    int resvg_parse_tree_from_data(const char *data,
                                   const size_t len,
                                   const resvg_options *opt,
                                   resvg_render_tree **tree);
    bool resvg_is_image_empty(const resvg_render_tree *tree);
    resvg_size resvg_get_image_size(const resvg_render_tree *tree);
    resvg_rect resvg_get_image_viewbox(const resvg_render_tree *tree);
    bool resvg_get_image_bbox(const resvg_render_tree *tree,
                              resvg_rect *bbox);
    bool resvg_node_exists(const resvg_render_tree *tree,
                           const char *id);
    bool resvg_get_node_transform(const resvg_render_tree *tree,
                                  const char *id,
                                  resvg_transform *ts);
    bool resvg_get_node_bbox(const resvg_render_tree *tree,
                             const char *id,
                             resvg_rect *bbox);
    void resvg_tree_destroy(resvg_render_tree *tree);
    void resvg_qt_render_to_canvas(const resvg_render_tree *tree,
                                   const resvg_options *opt,
                                   resvg_size size,
                                   void *painter);
    void resvg_qt_render_to_canvas_by_id(const resvg_render_tree *tree,
                                         const resvg_options *opt,
                                         resvg_size size,
                                         const char *id,
                                         void *painter);
}

static const char* toCStr(const QString &text)
{
    const auto utf8 = text.toUtf8();
    const auto data = utf8.constData();
    return qstrdup(data);
}

class Data
{
public:
    Data()
    {
        init();
    }

    ~Data()
    {
        clear();
    }

    void reset()
    {
        clear();
        init();
    }

    resvg_render_tree *tree = nullptr;
    resvg_options opt;
    qreal scaleFactor = 1.0;
    QRectF viewBox;
    QString errMsg;

private:
    void init()
    {
        resvg_init_options(&opt);

        // Do not set the default font via QFont::family()
        // because it will return a dummy one on Windows.
        // See https://github.com/RazrFalcon/resvg/issues/159

        opt.font_family = "Times New Roman";
        opt.languages = toCStr(QLocale().bcp47Name());
        opt.dpi = 96 * scaleFactor;
    }

    void clear()
    {
        // No need to deallocate opt.font_family, because it is a constant.

        if (tree) {
            resvg_tree_destroy(tree);
            tree = nullptr;
        }

        if (opt.path) {
            delete[] opt.path; // do not use free() because was allocated via qstrdup()
            opt.path = NULL;
        }

        if (opt.languages) {
            delete[] opt.languages; // do not use free() because was allocated via qstrdup()
            opt.languages = NULL;
        }

        viewBox = QRectF();
        errMsg = QString();
    }
};

static QString errorToString(const int err)
{
    switch (err) {
        case RESVG_OK :
            return QString();
        case RESVG_ERROR_NOT_AN_UTF8_STR :
            return QLatin1String("The SVG content has not an UTF-8 encoding.");
        case RESVG_ERROR_FILE_WRITE_FAILED :
            return QLatin1String("Failed to write to the file.");
        case RESVG_ERROR_INVALID_FILE_SUFFIX :
            return QLatin1String("Invalid file suffix.");
        case RESVG_ERROR_MALFORMED_GZIP :
            return QLatin1String("Not a GZip compressed data.");
        case RESVG_ERROR_INVALID_SIZE :
            return QLatin1String("SVG doesn't have a valid size.");
        case RESVG_ERROR_PARSING_FAILED :
            return QLatin1String("Failed to parse an SVG data.");
    }

    Q_UNREACHABLE();
}

} //ResvgPrivate

/**
 * @brief QSvgRenderer-like wrapper for resvg.
 */
class ResvgRenderer {
public:
    /**
     * @brief Constructs a new renderer.
     */
    ResvgRenderer();

    /**
     * @brief Constructs a new renderer and loads the contents of the SVG(Z) file.
     */
    ResvgRenderer(const QString &filePath);

    /**
     * @brief Constructs a new renderer and loads the SVG data.
     */
    ResvgRenderer(const QByteArray &data);

    /**
     * @brief Destructs the renderer.
     */
    ~ResvgRenderer();

    /**
     * @brief Loads the contents of the SVG(Z) file.
     */
    bool load(const QString &filePath);

    /**
     * @brief Loads the SVG data.
     */
    bool load(const QByteArray &data);

    /**
     * @brief Returns \b true if the file or data were loaded successful.
     */
    bool isValid() const;

    /**
     * @brief Returns an underling error when #isValid is \b false.
     */
    QString errorString() const;

    /**
     * @brief Checks that underling tree has any nodes.
     *
     * #ResvgRenderer and #ResvgRenderer constructors
     * will set an error only if a file does not exist or it has a non-UTF-8 encoding.
     * All other errors will result in an empty tree with a 100x100px size.
     *
     * @return Returns \b true if tree has any nodes.
     */
    bool isEmpty() const;

    /**
     * @brief Returns an SVG size.
     */
    QSize defaultSize() const;

    /**
     * @brief Returns an SVG size.
     */
    QSizeF defaultSizeF() const;

    /**
     * @brief Returns an SVG viewbox.
     */
    QRect viewBox() const;

    /**
     * @brief Returns an SVG viewbox.
     */
    QRectF viewBoxF() const;

    /**
     * @brief Returns bounding rectangle of the item with the given \b id.
     *        The transformation matrix of parent elements is not affecting
     *        the bounds of the element.
     */
    QRectF boundsOnElement(const QString &id) const;

    /**
     * @brief Returns bounding rectangle of a whole image.
     */
    QRectF boundingBox() const;

    /**
     * @brief Returns \b true if element with such an ID exists.
     */
    bool elementExists(const QString &id) const;

    /**
     * @brief Returns element's transform.
     */
    QTransform transformForElement(const QString &id) const;

    /**
     * @brief Sets the device pixel ratio for the image.
     */
    void setDevicePixelRatio(qreal scaleFactor);

    /**
     * @brief Renders the SVG data to canvas.
     */
    void render(QPainter *p) const;

    /**
     * @brief Renders the SVG data to \b QImage with a specified \b size.
     *
     * If \b size is not set, the \b defaultSize() will be used.
     */
    QImage renderToImage(const QSize &size = QSize()) const;

    /**
     * @brief Initializes the library log.
     *
     * Use it if you want to see any warnings.
     *
     * Must be called only once.
     *
     * All warnings will be printed to the \b stderr.
     */
    static void initLog();

private:
    QScopedPointer<ResvgPrivate::Data> d;
};

// Implementation.

inline ResvgRenderer::ResvgRenderer()
    : d(new ResvgPrivate::Data())
{
}

inline ResvgRenderer::ResvgRenderer(const QString &filePath)
    : d(new ResvgPrivate::Data())
{
    load(filePath);
}

inline ResvgRenderer::ResvgRenderer(const QByteArray &data)
    : d(new ResvgPrivate::Data())
{
    load(data);
}

inline ResvgRenderer::~ResvgRenderer() {}

inline bool ResvgRenderer::load(const QString &filePath)
{
    // Check for Qt resource path.
    if (filePath.startsWith(QLatin1String(":/"))) {
        QFile file(filePath);
        if (file.open(QFile::ReadOnly)) {
            return load(file.readAll());
        } else {
            return false;
        }
    }

    d->reset();

    d->opt.path = ResvgPrivate::toCStr(filePath);

    const auto err = resvg_parse_tree_from_file(d->opt.path, &d->opt, &d->tree);
    if (err != ResvgPrivate::RESVG_OK) {
        d->errMsg = ResvgPrivate::errorToString(err);
        return false;
    }

    const auto r = resvg_get_image_viewbox(d->tree);
    d->viewBox = QRectF(r.x, r.y, r.width, r.height);

    return true;
}

inline bool ResvgRenderer::load(const QByteArray &data)
{
    d->reset();

    const auto err = resvg_parse_tree_from_data(data.constData(), data.size(), &d->opt, &d->tree);
    if (err != ResvgPrivate::RESVG_OK) {
        d->errMsg = ResvgPrivate::errorToString(err);
        return false;
    }

    const auto r = resvg_get_image_viewbox(d->tree);
    d->viewBox = QRectF(r.x, r.y, r.width, r.height);

    return true;
}

inline bool ResvgRenderer::isValid() const
{
    return d->tree;
}

inline QString ResvgRenderer::errorString() const
{
    return d->errMsg;
}

inline bool ResvgRenderer::isEmpty() const
{
    if (d->tree)
        return !resvg_is_image_empty(d->tree);
    else
        return true;
}

inline QSize ResvgRenderer::defaultSize() const
{
    return defaultSizeF().toSize();
}

inline QSizeF ResvgRenderer::defaultSizeF() const
{
    if (d->tree)
        return d->viewBox.size();
    else
        return QSizeF();
}

inline QRect ResvgRenderer::viewBox() const
{
    return viewBoxF().toRect();
}

inline QRectF ResvgRenderer::viewBoxF() const
{
    if (d->tree)
        return d->viewBox;
    else
        return QRectF();
}

inline QRectF ResvgRenderer::boundsOnElement(const QString &id) const
{
    if (!d->tree)
        return QRectF();

    const auto utf8Str = id.toUtf8();
    const auto rawId = utf8Str.constData();
    ResvgPrivate::resvg_rect bbox;
    if (resvg_get_node_bbox(d->tree, rawId, &bbox))
        return QRectF(bbox.x, bbox.y, bbox.height, bbox.width);

    return QRectF();
}

inline QRectF ResvgRenderer::boundingBox() const
{
    if (!d->tree)
        return QRectF();

    ResvgPrivate::resvg_rect bbox;
    if (resvg_get_image_bbox(d->tree, &bbox))
        return QRectF(bbox.x, bbox.y, bbox.height, bbox.width);

    return QRectF();
}

inline bool ResvgRenderer::elementExists(const QString &id) const
{
    if (!d->tree)
        return false;

    const auto utf8Str = id.toUtf8();
    const auto rawId = utf8Str.constData();
    return resvg_node_exists(d->tree, rawId);
}

inline QTransform ResvgRenderer::transformForElement(const QString &id) const
{
    if (!d->tree)
        return QTransform();

    const auto utf8Str = id.toUtf8();
    const auto rawId = utf8Str.constData();
    ResvgPrivate::resvg_transform ts;
    if (resvg_get_node_transform(d->tree, rawId, &ts))
        return QTransform(ts.a, ts.b, ts.c, ts.d, ts.e, ts.f);

    return QTransform();
}

inline void ResvgRenderer::setDevicePixelRatio(qreal scaleFactor)
{
    d->scaleFactor = scaleFactor;
}

// TODO: render node

inline void ResvgRenderer::render(QPainter *p) const
{
    if (!d->tree)
        return;

    p->save();
    p->setRenderHint(QPainter::Antialiasing);

    const auto r = p->viewport();
    ResvgPrivate::resvg_size imgSize { (uint)r.width(), (uint)r.height() };
    resvg_qt_render_to_canvas(d->tree, &d->opt, imgSize, p);

    p->restore();
}

inline QImage ResvgRenderer::renderToImage(const QSize &size) const
{
    const auto s = size.isValid() ? size : defaultSize();
    QImage img(s, QImage::Format_ARGB32_Premultiplied);
    img.fill(Qt::transparent);

    QPainter p(&img);
    render(&p);
    p.end();

    return img;
}

inline void ResvgRenderer::initLog()
{
    ResvgPrivate::resvg_init_log();
}

#endif // RESVGQT_H