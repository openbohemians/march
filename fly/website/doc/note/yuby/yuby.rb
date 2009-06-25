
require 'yaml'

dy = YAML::load( File.new('numeric.yuby') )

dy.each_pair { |class_name, method_hash|
  klass = Object.const_get(class_name)
  p klass
  method_hash.each_pair { |method_name, method_def|
    klass.class_eval %{
      def #{method_name}
        #{method_def}
      end
    }
  }
}


# test

p 1.byte
