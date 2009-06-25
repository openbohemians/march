
require 'yaml'
require 'rbconfig'

#require 'fly/error'
require 'fly/kernel'


module Fly

  CORE_DIR = File.join( Config::CONFIG['datadir'], 'fly', 'core' )
  CORE_PARTS = [ 'console', 'math' ]

  module Compiler

    def load( io )
      case io
      when IO
        compile( io.read )
      else
        compile( io )
      end
    end

    # Program compile is a two stage process.

    def compile( code )
      code = compile_kernel( code )
      compile_types( YAML.load( code ) )
      mdl = compile_model( YAML.load( code ), nil ) #Kernel.model )
      Program.new( mdl )
    end

    def compile_kernel( code )
      core = load_kernel
      # Build user program with core components
      str = "---\n"
      str << core
      str << "\n\n# USER PROGRAM\n\n"
      str << yaml_clean( code.strip )
      str << "\n\n"
      # Display if debug mode
      #puts str if $DEBUG
      # Load yaml
      str
    end

    # Load core components

    def load_kernel
      core = ""
      #core << yaml_clean( File.read( File.join(CORE_DIR, 'kernel') ) )
      CORE_PARTS.each { |name|
        str = File.read( File.join(CORE_DIR, name) )
        core << "\n\n" << yaml_clean( str.strip )
      }
      return core
    end

    def yaml_clean( str )
      str.gsub(/\%YAML\s+\d+[.]\d+/, '').gsub('---', '').strip
    end

    # Precompile sets up all type definitions.

    def compile_types( yml, prekey=nil )
      yml.each { |key, val|
        current_key = prekey ? "#{prekey}.#{key}" : key
        case val
        when Type
          val.compile( current_key )
        when Hash
          compile_types( val, current_key  )
        end
      }
    end

    # Compile

    def compile_model( yml, parent_node=nil )

      model = Model.new
      parent_node ||= Node.new( 'TOPLEVEL', nil, :sub, model, nil )

      yml.each do |key, val|
        name, face = parse( key )

        case val
          when Type
            # do nothing
          when Hash
            node = Node.new( name, face, :sub, nil, parent_node )
            node.value = compile_model( val, node )
          when Fn
            node = Node.new( name, face, :fn , val, parent_node )
          when RubyFn
            node = Node.new( name, face, :rb , val, parent_node )
          when Integer
            node = Node.new( name, face, :int, val, parent_node )
          when Float
            node = Node.new( name, face, :num, val, parent_node )
          when String
            node = Node.new( name, face, :str, val, parent_node )
          when Array
            node = Node.new( name, face, :ary, val, parent_node )
        else
          # TODO user defined types
          puts "Error: Uknown type #{val.class}."
        end

        model.new_word( name, face, node )
      end

      return model
    end

    #

    def parse( key )
      i = key.index '('
      if i
        word = key[0...i].strip
        face = key.strip[i+1...-1]
      else
        word = key.strip
        face = nil
      end
      face = parse_interface( face )
      return word, face
    end

    #

    def parse_interface( face )
      return [] unless face
      face.strip.split(' ').collect do |w|
        case w
        when 'n', 'num', 'number', 'numeric'
          Numeric
        when 'i', 'int', 'integer'
          Integer
        when 's', 'str', 'string'
          String
        when 'a', 'ary', 'array'
          Array
        when 'itr', 'iterator'

        when 'o', 'obj', 'object'
          Object
        else
          Object
        end
      end
    end

  end


  class Program

    extend Compiler

    include Kernel

    attr_reader :stack, :mode

    def initialize( model )
      @model = model

      @mode = :dec
      @stack = []   # parameter stack
      @return = []  # return stack
    end

    def start
      resolve( @model.start )
    end

    def resolve( node )
      case node.type
      when :fn
        calc( node )
      when :ruby
        send( node.value )
      else
        node.value
      end
    end

    def calc( node ) #, *args )
      locals = {}
      code = node.value
      terms = shellwords(code)
      #terms = code.downcase.split(/\s+/)  # must imporve to handle strings and blocks
      terms.each do |term|
#puts( (stack.reverse << term).join(' ') ) #if $DEBUG
        r = nil
        case term
          when Symbol
            term = term.to_s
            case term
              when /^\&/
                # memory
                locals[term.sub("&",'')] = stack.shift
              when /^\*/
                # get memory
                r = locals[term.sub("*",'')]
              when /^\.$/
                puts drop
              #when /\./
              #  parts = term.split('.')
            else
              # lookup
              n = node.lookup( term.to_s, stack )
              if n
                r = resolve( n )
              else
                puts "ERROR: #{term}"
                #begin
                  #Kernel.words( token )
                #rescue
                #  puts "No method error -- #{token}"
                #  exit 1
                #end
              end
            end
          #when Array #substack
          #  r = term
          #when Numeric
            #when /^-?(?:\d[,_\d]*|0[,_0-7]+|0x[,_0-9a-f]+|0b[,_01]+)$/,
            #     /^-?\d[,_\d]*\.\d[,_\d]*(?:e-?[,_\d]+)?$/
            # number
            #r = instance_eval( term.tr(",", "") )
          #  r = term
          #when String #/^\".*\"$/
            # string
          #  r = term #[1..-1]
        else
          r = term
        end
        stack.unshift r unless r.nil?
      end

      nil #stack.first
    end

    #
    # Split text into an array of tokens in the same way the UNIX Bourne
    # shell does.
    #
    # See the +Shellwords+ module documentation for an example.
    #
    def shellwords(line)
      line = String.new(line) rescue
        raise(ArgumentError, "Argument must be a string")
      line.lstrip!
      words = []
      until line.empty?
        field = ''
        loop do
          if line.sub!(/\A"(([^"\\]|\\.)*)"/, '') then
            # string (double-quote)
            snippet = $1.gsub(/\\(.)/, '\1')
          elsif line =~ /\A"/ then
            raise ArgumentError, "Unmatched double quote: #{line}"
          elsif line.sub!(/\A'([^']*)'/, '') then
            # string (single-quote)
            snippet = $1
          elsif line =~ /\A'/ then
            raise ArgumentError, "Unmatched single quote: #{line}"
          elsif line.sub!(/\A\[([^\[\]]*)\]/, '') then
            # stack (array)
            field = shellwords($1)
            line.lstrip!
            break
          elsif line =~ /\A[\[\]]/
            raise ArgumentError, "Unmatched stack bracket: #{line}"
          #elsif line.sub!(/\A\{([^\{\}]*)\}/, '') then
          #  field = shellwords($1)
          #  line.lstrip!
          #  break
          #elsif line =~ /\A[\{\}]/ then
          #  raise ArgumentError, "Unmatched block bracket: #{line}"
          elsif line.sub!(/\A\\(.)?/, '')
            # ?
            snippet = $1 || '\\'
          elsif line.sub!(/\A([^\s\\'"]+)/, '')
            # word
            #snippet = $1.to_sym
            sym = $1
            if /^-?(?:\d[,_\d]*|0[,_0-7]+|0x[,_0-9a-f]+|0b[,_01]+)$/ =~ sym or
               /^-?\d[,_\d]*\.\d[,_\d]*(?:e-?[,_\d]+)?$/ =~ sym
              field = instance_eval( sym.tr(",", "") )
            else
              field = sym.to_sym
            end
            line.lstrip!
            break
          else
            line.lstrip!
            break
          end
          field.concat(snippet)
        end
        words.push(field)
      end
      words
    end

  end

end #module FLY
